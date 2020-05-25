extern crate attohttpc;
extern crate clap;
#[macro_use]
extern crate anyhow;

use anyhow::Error;
use byteorder::{LittleEndian, ReadBytesExt};
use clap::{App, Arg};
use indicatif::{ProgressBar, ProgressStyle};
use std::env::{split_paths, var, var_os};
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;

fn init() -> Result<(), Error> {
    let matches = App::new("upder")
        .version("0.0.1")
        .author("Alexander Adhyatma <alex@asiatech.dev>")
        .about("Updates flutter, rust-analyzer, rustup, via systemd-timer (user)")
        .args(&[Arg::with_name("gen_systemd")
            .help("Generate systemd timer file")
            .long("gen-systemd")
            .short("g")
            .takes_value(false)
            .required(false)])
        .get_matches();

    if matches.is_present("gen_systemd") {
        return generate_systemd_timer();
    }
    return Ok(());
}

// Wrapper for shell commnands
fn exec_cmd(cmd: &str, args: &[&str]) -> Result<String, Error> {
    let the_cmd = Command::new(cmd)
        .args(args)
        .spawn()
        .ok()
        .expect("failed to execute");
    match the_cmd.wait_with_output() {
        Ok(out) => {
            let o = String::from_utf8_lossy(&out.stdout);
            return Ok(o.to_string());
        }
        Err(e) => return Err(format_err!("Error {}", e)),
    }
}

// indicatif progress bar
fn fetch_download(uri: &str, dest: &Path) -> Result<String, Error> {
    let resp = attohttpc::get(uri).send()?;
    let total_size: Result<&attohttpc::header::HeaderValue, Error> = {
        if resp.is_success() {
            Ok(resp
                .headers()
                .get(attohttpc::header::CONTENT_LENGTH)
                .unwrap())
        } else {
            Err(format_err!(
                "error getting content length of uri: {} => {}",
                uri,
                resp.status()
            ))
        }
    };
    let mut tot = total_size?.as_bytes();
    let pb = ProgressBar::new(tot.read_u64::<LittleEndian>().unwrap());
    pb.set_style(ProgressStyle::default_bar().template(
        "{spinner: .green} [{elapsed_precise}] [{bar:40.yellow/cyan}] {bytes/total_bytes} ({eta})",
    ).progress_chars("#>-"));

    let dest_file = File::create(dest)?;

    resp.write_to(dest_file)?;

    Ok(format!("Successfully downloaded: {} to {:?}", uri, dest))
}

// Find executable, return its path
fn find_exe<P>(bin_name: P) -> Option<PathBuf>
where
    P: AsRef<Path>,
{
    var_os("PATH").and_then(|paths| {
        split_paths(&paths)
            .filter_map(|dir| {
                let full_path = dir.join(&bin_name);
                if full_path.is_file() {
                    Some(full_path)
                } else {
                    None
                }
            })
            .next()
    })
}

fn exe_exists(bin_name: &str) -> Result<PathBuf, Error> {
    match find_exe(bin_name) {
        Some(p) => return Ok(p),
        None => return Err(format_err!("Error: {} not found", bin_name)),
    }
}

// Generates systemd timer and install it as user unit.
fn generate_systemd_timer() -> Result<(), Error> {
    let config_xdg = var("XDG_CONFIG_HOME")?;
    let systemd_user_path = Path::new(&config_xdg).join("systemd").join("user");
    create_dir_all(&systemd_user_path)?;
    let raw_string_service = format!(
        r#"
[Unit]
Description=Updates flutter, rust-analyzer, rustup

[Service]
Type=oneshot
ExecStart={}
StandardOutput=journal"#,
        get_bin_path()?
    );
    let raw_string_timer = r#"
[Unit]
Description=Updates flutter, rust-analyzer, rustup

[Timer]
OnCalendar=daily
Persistent=true

[Install]
WantedBy=timers.target"#;

    let service_file_path = &systemd_user_path.join("upder.service");
    let timer_file_path = &systemd_user_path.join("upder.timer");
    let mut svc_file = File::create(&service_file_path)?;
    let mut file = File::create(&timer_file_path)?;
    svc_file.write_all(raw_string_service.as_bytes())?;
    file.write_all(raw_string_timer.as_bytes())?;
    let out = exec_cmd("systemctl", &["--user", "enable", "--now", "upder.timer"])?;
    println!("{}", out);

    Ok(())
}

fn update_flutter() -> Result<(), Error> {
    exe_exists("flutter")?;
    let out = exec_cmd("flutter", &["upgrade", "--verbose"])?;
    println!("{}", out);
    Ok(())
}

fn update_rustup() -> Result<(), Error> {
    exe_exists("rustup")?;
    let mut out = exec_cmd("rustup", &["self", "update"])?;
    println!("{}", out);
    out = exec_cmd("rustup", &["self", "upgrade-data"])?;
    println!("{}", out);
    out = exec_cmd("rustup", &["update"])?;
    println!("{}", out);
    Ok(())
}

fn get_bin_path() -> Result<String, Error> {
    let home_path = var("HOME")?;
    Ok([home_path.as_str(), "/.local/bin/", "rust-analyzer"].concat())
}

fn update_rust_analyzer() -> Result<(), Error> {
    let home_path = var("HOME")?;
    let bin_path = Path::new(&home_path).join(".local/bin");
    let bin_path_string = get_bin_path()?;
    create_dir_all(&bin_path)?;
    let analyzer_url = "https://github.com/rust-analyzer/rust-analyzer/releases/download/nightly/rust-analyzer-mac";
    let analyzer_file_path = Path::new(&bin_path_string);
    fetch_download(analyzer_url, analyzer_file_path)?;
    //    let out = exec_cmd("chmod", &["+x", get_bin_path()?.as_ref()])?;
    //    println!("{}", out);

    Ok(())
}

fn main() -> Result<(), Error> {
    init()?;
    update_rustup()?;
    update_flutter()?;
    update_rust_analyzer()?;
    Ok(())
}

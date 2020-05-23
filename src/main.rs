extern crate clap;
extern crate ureq;
#[macro_use] extern crate failure;

use clap::{App, Arg};
use std::fs::{create_dir_all, File};
use std::io::{prelude::*};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::env::{var, var_os, split_paths};
use failure::Error;

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
        return generate_systemd_timer()
    }
    return Ok(())
}

// Wrapper for shell commnands
fn exec_cmd(cmd: &str, args: &[&str]) -> Result<String, Error> {
    let the_cmd = Command::new(cmd)
        .args(args)
        .spawn().ok().expect("failed to execute");
    match the_cmd.wait_with_output() {
        Ok(out) => {
            let o = String::from_utf8_lossy(&out.stdout);
            return Ok(o.to_string())
        },
        Err(e) =>  return Err(format_err!("Error {}", e))
    }
}

// Find executable, return its path
fn find_exe<P>(bin_name: P) -> Option<PathBuf>
where P: AsRef<Path> {
    var_os("PATH").and_then(|paths| {
        split_paths(&paths).filter_map(|dir| {
            let full_path = dir.join(&bin_name);
            if full_path.is_file() {
                Some(full_path)
            } else {
                None
            }
        }).next()
    })
}

fn exe_exists(bin_name: &str) -> Result<PathBuf, Error> {
    match find_exe(bin_name) {
        Some(p) => return Ok(p),
        None => return Err(format_err!("Error: {} not found", bin_name))
    }
}


// Generates systemd timer and install it as user unit.
fn generate_systemd_timer() -> Result<(), Error>{
    let config_xdg = var("XDG_CONFIG_HOME")?;
    let systemd_user_path = Path::new(&config_xdg).join("systemd").join("user");
    create_dir_all(&systemd_user_path)?;
    let raw_string_service = format!(r#"
[Unit]
Description=Updates flutter, rust-analyzer, rustup

[Service]
Type=oneshot
ExecStart={}
StandardOutput=journal"#, get_bin_path()?);
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
    let mut file=  File::create(&timer_file_path)?;
    svc_file.write_all(raw_string_service.as_bytes())?;
    file.write_all(raw_string_timer.as_bytes())?;
    let out = exec_cmd("systemctl", &[
                   "--user", "enable", "--now", "upder.timer",
    ])?;
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
    create_dir_all(&bin_path)?;
    let analyzer_url = "https://github.com/rust-analyzer/rust-analyzer/releases/download/nightly/rust-analyzer-linux";
    let res = ureq::get(analyzer_url).timeout_connect(2_000).redirects(10).call();
    let analyzer_file_path = &bin_path.join("rust-analyzer");
    let mut f = File::create(&analyzer_file_path)?;
    let str_body = res.into_string()?;
    let mut body = str_body.as_ref();
    f.write_all(&mut body)?;
    let out = exec_cmd("chmod", &["+x", get_bin_path()?.as_ref()])?;
    println!("{}", out);

    Ok(())
}

fn main() -> Result<(), Error> {
    init()?;
    update_rustup()?;
    update_flutter()?;
    update_rust_analyzer()?;
    Ok(())
}

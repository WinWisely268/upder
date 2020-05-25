#[macro_use]
extern crate anyhow;

use anyhow::Error;
use structopt::StructOpt;
use std::env::{split_paths, var, var_os};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
// use tokio::{fs::{create_dir_all, OpenOptions, File}, io::AsyncWriteExt};
use std::fs::{create_dir_all, File};

#[derive(StructOpt, Debug)]
#[structopt(name = "upder")]
struct CmdOpt {
    // option to generate systemd timer unit
    #[structopt(short = "g", long = "gen", parse(try_from_str))]
    gen: bool,
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

fn fetch_github(uri: &str, dest: &str) -> Result<String, Error> {
    let out = exec_cmd("aria2c", &[uri, "-o", dest])?;
    println!("{}",out);
    exec_cmd("chmod", &["+x", get_bin_path()?.as_ref()])
}

// build headers for each request sent
// fn build_headers() -> header::HeaderMap {
//     let mut headers = header::HeaderMap::new();
//     headers.insert(
//         header::USER_AGENT,
//         "Mozilla/5.0 (X11; OpenSUSE; Linux x86_64; rv:75.0) Gecko/20100101 Firefox/75.0"
//         .parse()
//         .expect("Invalid UA"),
//         );
//     headers.insert(
//         "Accept",
//         "application/octet-stream".parse().expect("Invalid accept type"),
//         );
//     headers
// }


// download uri with indicatif progress bar
// async fn fetch_github(uri: &str, dest: &Path) -> Result<String, Error> {
//     let client = Client::new();
//     let total_size = {
//         println!("Getting HEAD response from {}", uri);
//         let resp = client.head(uri).
//             headers(build_headers()).send().await?;
//         if resp.status().is_success() {
//             Ok(resp.headers().get(header::CONTENT_LENGTH).and_then(|l| l.to_str().ok())
//                .and_then(|l| l.parse().ok()).unwrap_or(0))
//         } else {
//             Err(anyhow!("Failed to download URL: {}, Err: {:?}", uri, resp.status()))
//         }
//     };
//     let tot = total_size?;
//     let pb = ProgressBar::new(tot);
//     pb.set_style(ProgressStyle::default_bar().template(
//             "{spinner: .yellow} [{elapsed_precise}] [{bar:60.green/black}] {bytes/total_bytes} ({eta})",
//             ).progress_chars("#>-"));

//     let mut req = client.get(uri).headers(build_headers());
//     if dest.exists() {
//         let size = dest.metadata()?.len().saturating_sub(1);
//         req = req.header(header::RANGE, format!("bytes={}-", size));
//         pb.inc(size);
//     }

//     let mut resp = req.send().await?;
//     let mut dest_file = OpenOptions::new()
//         .create(true).append(false).open(dest).await?;
//     while let Some(chunk) = resp.chunk().await? {
//         dest_file.write_all(&chunk).await?;
//         pb.inc(chunk.len() as u64);
//     }

//     Ok(format!("Successfully downloaded: {} to {:?}", uri, dest))
// }

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
    let analyzer_url = "https://github.com/rust-analyzer/rust-analyzer/releases/download/nightly/rust-analyzer-linux";
    let out = fetch_github(analyzer_url, &bin_path_string)?;
    println!("{}", out);

    Ok(())
}

fn main() -> Result<(), Error> {
    let opt = CmdOpt::from_args();
    if opt.gen {
        generate_systemd_timer()?
    }
    update_rustup()?;
    update_flutter()?;
    update_rust_analyzer()?;
    Ok(())
}

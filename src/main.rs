#[macro_use]
extern crate anyhow;

use anyhow::Error;
use indicatif::{FormattedDuration, HumanBytes, ProgressBar, ProgressStyle};
use isahc::prelude::*;
use structopt::StructOpt;
use std::env::{split_paths, var, var_os};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs::{remove_file, create_dir_all, File, OpenOptions};
use sys_info::os_type;

#[derive(Clone)]
struct Url {
    uri: String,
}

impl Url {
    fn new() -> Self {
        Url{
            uri: "".to_string(),
        }
    }

    fn get_url(mut self) -> Result<Self, Error> {
        let uri = self.build_url()?;
        self.uri = uri;
        Ok(self)

    }

    fn build_url(&self) -> Result<String, Error> {
        let osname = os_type()?;
        let platform = match &*osname {
            "Linux" => "linux",
            "Darwin" => "mac",
            _ => "linux",
        };

        let bin_name = "rust-analyzer";
        Ok(format!(
                "https://github.com/{}/{}/releases/download/nightly/{}-{}",
                bin_name,
                bin_name,
                bin_name,
                platform,
                ))
    }
}

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

fn fetch_url(uri: &str, dest: &str) -> Result<(), Error> {
    let bar = ProgressBar::new(0).with_style(
        ProgressStyle::default_bar().template(
            "{bar:60.cyan/blue} {bytes:>7}/{total_bytes:7} ({eta}) {msg}"
            ),
            );
    let client = HttpClient::builder().metrics(true)
        .redirect_policy(isahc::config::RedirectPolicy::Limit(10))
        .build()?;
    let mut res = client.get(uri)?;
    let metrics = res.metrics().unwrap().clone();
    let body = res.body_mut();
    let mut buf = [0; 16384 * 4];
    let mut dest_file = OpenOptions::new().create_new(true)
        .append(true).truncate(true).open(dest)?;

    loop {
        match body.read(&mut buf) {
            Ok(0) => {
                bar.finish();
                break;
            }
            Ok(s) => {
                dest_file.write_all(&buf[..s])?;
                bar.set_position(metrics.download_progress().0);
                bar.set_length(metrics.download_progress().1);
                bar.set_message(
                    &format!(
                        "time: {} speed: {}/sec",
                        FormattedDuration(metrics.total_time()),
                        HumanBytes(metrics.download_speed() as u64),
                        ),
                        );
            }
            Err(e) => {
                bar.finish_at_current_pos();
                eprintln!("Error: {}", anyhow!(e));
                return Ok(());
            }
        }
    }
    Ok(())
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

fn exe_bool_exists(bin_name: &str) -> bool {
    match find_exe(bin_name) {
        Some(_) => true,
        None => false,
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
    let bin_path_string = get_bin_path()?;
    if exe_bool_exists("rust-analyzer") {
        remove_file(Path::new(&bin_path_string))?;
    };
    let analyzer_uri = Url::new().get_url()?;
    fetch_url(&analyzer_uri.uri, &bin_path_string)?;
    exec_cmd("chmod", &["+x", &bin_path_string])?;
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

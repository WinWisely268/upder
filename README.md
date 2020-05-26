# UPDER

Updates `flutter`, `rustup` (and `rustc`), and `rust-analyzer` installs.

## Usage

```
upder --help
upder 0.1.0

USAGE:
    upder --gen <gen>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -g, --gen <gen>
```

Flag `-g` is used to create `systemd-timer` (linux-only).

### TODO:

- [ ] Generate brew services / `crontab` for MacOS
- [ ] Windows support ? 

// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `aegis-vulcan-gpu`: CLI tool for GPU host record, DMA-BUF share, and VFIO BAR mapping (M25).
//!
//! Usage:
//!   `aegis-vulcan-gpu record [--sysfs ROOT] [--dev ROOT] [--json PATH]`
//!   `aegis-vulcan-gpu share [--sysfs ROOT] [--dev ROOT]`
//!   `aegis-vulcan-gpu map-bar --device <PCI addr> [--sysfs ROOT] [--dev ROOT]`

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use aegis_vulcan_gpu::{run_map_bar, run_record, run_share};

enum Subcommand {
    Record { json_out: Option<PathBuf> },
    Share,
    MapBar { device: String },
}

struct CliConfig {
    subcommand: Subcommand,
    sysfs: PathBuf,
    dev: PathBuf,
}

fn main() -> ExitCode {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let config = match parse_args() {
        Ok(cfg) => cfg,
        Err(usage) => {
            let _ = writeln!(out, "{usage}");
            return ExitCode::from(64);
        }
    };

    match &config.subcommand {
        Subcommand::Record { json_out } => execute_record(&config, json_out.as_deref(), &mut out),
        Subcommand::Share => execute_share(&config, &mut out),
        Subcommand::MapBar { device } => execute_map_bar(&config, device, &mut out),
    }
}

fn parse_args() -> Result<CliConfig, String> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: aegis-vulcan-gpu record [--sysfs ROOT] [--dev ROOT] [--json PATH]\n       \
                 aegis-vulcan-gpu share [--sysfs ROOT] [--dev ROOT]\n       \
                 aegis-vulcan-gpu map-bar --device <PCI addr> [--sysfs ROOT] [--dev ROOT]";
    let subcmd_str = args.next();
    let is_record = subcmd_str.as_deref() == Some("record");
    let is_share = subcmd_str.as_deref() == Some("share");
    let is_map_bar = subcmd_str.as_deref() == Some("map-bar");
    if !is_record && !is_share && !is_map_bar {
        return Err(usage.to_owned());
    }

    let mut sysfs = PathBuf::from("/sys");
    let mut dev = PathBuf::from("/dev");
    let mut json_out = None;
    let mut device = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--sysfs" => sysfs = PathBuf::from(flag_value(&mut args, "--sysfs", "a path")?),
            "--dev" => dev = PathBuf::from(flag_value(&mut args, "--dev", "a path")?),
            "--json" if is_record => {
                json_out = Some(PathBuf::from(flag_value(&mut args, "--json", "a path")?));
            }
            "--device" if is_map_bar => {
                device = Some(flag_value(&mut args, "--device", "a PCI address")?);
            }
            other => return Err(format!("error: unrecognized flag '{other}'")),
        }
    }

    let subcommand = if is_record {
        Subcommand::Record { json_out }
    } else if is_share {
        Subcommand::Share
    } else {
        let dev_addr =
            device.ok_or_else(|| "error: map-bar requires --device <PCI addr>".to_owned())?;
        Subcommand::MapBar { device: dev_addr }
    };

    Ok(CliConfig {
        subcommand,
        sysfs,
        dev,
    })
}

/// Takes the value that follows `flag`, or says which kind of argument it requires.
fn flag_value(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
    kind: &str,
) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("error: {flag} requires {kind} argument"))
}

fn execute_record(
    config: &CliConfig,
    json_out: Option<&Path>,
    out: &mut std::io::StdoutLock<'_>,
) -> ExitCode {
    let outcome = run_record(&config.sysfs, &config.dev);
    match outcome {
        Ok(run) => {
            for line in &run.lines {
                let _ = writeln!(out, "{line}");
            }
            if let Some(json_path) = json_out
                && let Err(err) = write_json_document(json_path, &run)
            {
                let _ = writeln!(out, "FAIL: could not write JSON output: {err}");
                return ExitCode::from(1);
            }
            if run.is_pass() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(err) => {
            let _ = writeln!(out, "FAIL: {err}");
            ExitCode::from(1)
        }
    }
}

fn execute_share(config: &CliConfig, out: &mut std::io::StdoutLock<'_>) -> ExitCode {
    match run_share(&config.sysfs, &config.dev) {
        Ok(run) => {
            for line in &run.lines {
                let _ = writeln!(out, "{line}");
            }
            if run.failed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(err) => {
            let _ = writeln!(out, "FAIL: {err}");
            ExitCode::from(1)
        }
    }
}

fn execute_map_bar(
    config: &CliConfig,
    device: &str,
    out: &mut std::io::StdoutLock<'_>,
) -> ExitCode {
    let run = run_map_bar(&config.sysfs, &config.dev, device);
    for line in &run.lines {
        let _ = writeln!(out, "{line}");
    }
    if run.failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn write_json_document(
    path: &Path,
    run: &aegis_vulcan_gpu::RecordRun,
) -> Result<(), std::io::Error> {
    let json = serde_json::to_string_pretty(&run.document)?;
    std::fs::write(path, json)
}

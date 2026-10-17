// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `aegis-vulcan-gpu`: CLI tool for GPU host record and verification (M25).
//!
//! Usage:
//!   `aegis-vulcan-gpu record [--sysfs ROOT] [--dev ROOT] [--json PATH]`

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use aegis_vulcan_gpu::run_record;

struct CliConfig {
    sysfs: PathBuf,
    dev: PathBuf,
    json_out: Option<PathBuf>,
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

    execute_record(&config, &mut out)
}

fn parse_args() -> Result<CliConfig, String> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: aegis-vulcan-gpu record [--sysfs ROOT] [--dev ROOT] [--json PATH]";
    let Some(cmd) = args.next() else {
        return Err(usage.to_owned());
    };
    if cmd != "record" {
        return Err(usage.to_owned());
    }

    let mut sysfs = PathBuf::from("/sys");
    let mut dev = PathBuf::from("/dev");
    let mut json_out = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--sysfs" => {
                let val = args
                    .next()
                    .ok_or_else(|| "error: --sysfs requires a path argument".to_owned())?;
                sysfs = PathBuf::from(val);
            }
            "--dev" => {
                let val = args
                    .next()
                    .ok_or_else(|| "error: --dev requires a path argument".to_owned())?;
                dev = PathBuf::from(val);
            }
            "--json" => {
                let val = args
                    .next()
                    .ok_or_else(|| "error: --json requires a path argument".to_owned())?;
                json_out = Some(PathBuf::from(val));
            }
            other => return Err(format!("error: unrecognized flag '{other}'")),
        }
    }

    Ok(CliConfig {
        sysfs,
        dev,
        json_out,
    })
}

fn execute_record(config: &CliConfig, out: &mut std::io::StdoutLock<'_>) -> ExitCode {
    let outcome = run_record(&config.sysfs, &config.dev);
    match outcome {
        Ok(run) => {
            for line in &run.lines {
                let _ = writeln!(out, "{line}");
            }
            if let Some(json_path) = &config.json_out
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

fn write_json_document(
    path: &Path,
    run: &aegis_vulcan_gpu::RecordRun,
) -> Result<(), std::io::Error> {
    let json = serde_json::to_string_pretty(&run.document)?;
    std::fs::write(path, json)
}

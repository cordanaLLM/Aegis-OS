// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `aegis-vesta-guest`: the init of every M21 microVM.
//!
//! Built statically (`-C target-feature=+crt-static`) by
//! `tools/verify_workstation.py` and packed as `/init` into the initramfs.
//! It refuses to run as anything but process 1, so it can never restart the
//! host: outside a microVM it prints why and exits 64. Inside, it serves
//! candidate evaluations over `AF_VSOCK` until its bound or its accept budget,
//! then restarts the guest, which Firecracker answers by exiting.

use std::io::Write;
use std::process::ExitCode;

use aegis_vesta_sandbox::serve;
use rustix::system::{RebootCommand, reboot};

fn main() -> ExitCode {
    let stderr = std::io::stderr();
    let mut log = stderr.lock();
    if std::process::id() != 1 {
        let _ = writeln!(
            log,
            "aegis-vesta-guest: not process 1; it runs only as a microVM's init"
        );
        return ExitCode::from(64);
    }
    match serve(&mut log) {
        Ok(served) => {
            let _ = writeln!(
                log,
                "aegis-vesta-guest: answered {served} connection(s); restarting"
            );
        }
        Err(error) => {
            let _ = writeln!(log, "aegis-vesta-guest: {error}; restarting");
        }
    }
    let _ = log.flush();
    match reboot(RebootCommand::Restart) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(log, "aegis-vesta-guest: restart refused: {error}");
            ExitCode::from(1)
        }
    }
}

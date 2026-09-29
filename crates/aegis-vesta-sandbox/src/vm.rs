// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! One Firecracker process on the host: started, reached, stopped.
//!
//! A [`RunningVm`] exists only for a row [`aegis_vesta::MicroVmController`]
//! admitted: [`RunningVm::start`] takes the controller's
//! [`MicroVmInstance`], so the context identifier the guest holds is the one
//! the controller derived, and a request the controller refused never reaches
//! a process. Every wait is bounded twice, by a poll count and by a budget
//! (HISS-02), and a [`RunningVm`] that is dropped without [`RunningVm::stop`]
//! still kills and reaps its process.

use std::fs::{self, File};
use std::io::{ErrorKind, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use aegis_vesta::{
    CandidateEvaluation, GuestMemoryMib, MAX_CONTRACT_PAYLOAD_BYTES, MicroVmInstance, VmId,
    VsockCid,
};

use crate::config::{VmConfig, check_socket_path, render};
use crate::error::SandboxError;
use crate::evaluate::check_answer;
use crate::line::{MAX_ACK_BYTES, Unframed, connect_command, deadline_error, parse_ack, read_line};
use crate::request::{EVALUATION_PORT, EvaluationRequest};

/// How long a microVM has from its process start to its guest accepting.
pub const BOOT_BUDGET: Duration = Duration::from_secs(10);

/// How long one handshake read may take.
pub const HANDSHAKE_BUDGET: Duration = Duration::from_secs(1);

/// How long one request and its answer may take.
pub const EXCHANGE_BUDGET: Duration = Duration::from_secs(5);

/// How long a killed process has to be reaped.
pub const STOP_BUDGET: Duration = Duration::from_secs(5);

/// The pause between two polls of a process or a socket.
pub const POLL: Duration = Duration::from_millis(10);

/// Scalar upper bound on the polls of one wait: 10 s at 10 ms.
pub const MAX_POLLS: u32 = 1_000;

/// Scalar upper bound, in bytes, on one answer line: the contract's payload
/// bound and its newline.
pub const MAX_ANSWER_BYTES: usize = MAX_CONTRACT_PAYLOAD_BYTES + 1;

/// Scalar upper bound on the entries a microVM directory is swept for sockets.
pub const MAX_DIRECTORY_ENTRIES: usize = 64;

/// What every microVM of one run is started from.
#[derive(Debug, Clone, Copy)]
pub struct Launch<'a> {
    /// The fetched, pinned Firecracker binary.
    pub firecracker: &'a Path,
    /// The pinned guest kernel.
    pub kernel: &'a Path,
    /// The initramfs holding the guest init.
    pub initrd: &'a Path,
    /// The run directory each microVM gets a subdirectory of.
    pub run_dir: &'a Path,
}

/// How a microVM's process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stopped {
    /// The sandbox that was stopped.
    pub vm_id: VmId,
    /// The process's exit status, once reaped.
    pub status: ExitStatus,
    /// How many socket files were removed from its directory.
    pub sockets_removed: usize,
}

/// One started Firecracker process and the sandbox it runs.
#[derive(Debug)]
pub struct RunningVm {
    vm_id: VmId,
    cid: VsockCid,
    dir: PathBuf,
    socket: PathBuf,
    child: Child,
    reaped: bool,
    started: Instant,
}

impl RunningVm {
    /// Starts the microVM for the controller's row `instance`.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxError::Io`] when the directory, the configuration or
    /// the console cannot be written or the process cannot be started, and
    /// [`SandboxError::Refused`] for a path the configuration refuses.
    pub fn start(launch: &Launch<'_>, instance: &MicroVmInstance) -> Result<Self, SandboxError> {
        let vm_id = instance.vm_id;
        let dir = launch.run_dir.join(format!("vm-{}", vm_id.get()));
        let socket = dir.join("v.sock");
        check_socket_path(&socket)?;
        fs::create_dir(&dir)
            .map_err(|error| SandboxError::io("creating a microVM directory", error))?;
        let config = render(&VmConfig {
            kernel: launch.kernel,
            initrd: launch.initrd,
            memory: instance.memory,
            cid: instance.vsock_cid,
            socket: &socket,
        })?;
        let config_path = dir.join("vm.json");
        fs::write(&config_path, config)
            .map_err(|error| SandboxError::io("writing a microVM configuration", error))?;
        let child = spawn(launch.firecracker, &config_path, &dir, vm_id)?;
        Ok(Self {
            vm_id,
            cid: instance.vsock_cid,
            dir,
            socket,
            child,
            reaped: false,
            started: Instant::now(),
        })
    }

    /// Returns the sandbox this process runs.
    #[must_use]
    pub const fn vm_id(&self) -> VmId {
        self.vm_id
    }

    /// Returns the guest's context identifier.
    #[must_use]
    pub const fn cid(&self) -> VsockCid {
        self.cid
    }

    /// Returns the microVM's directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Returns `true` while the process has not exited.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxError::Io`] when the process cannot be polled.
    pub fn alive(&mut self) -> Result<bool, SandboxError> {
        let polled = self
            .child
            .try_wait()
            .map_err(|error| SandboxError::io("polling a firecracker process", error))?;
        Ok(polled.is_none())
    }

    /// Sends one request to the guest and returns its checked answer.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxError`] when the guest does not accept within
    /// [`BOOT_BUDGET`], the exchange passes [`EXCHANGE_BUDGET`], the guest
    /// answers `ERROR`, the payload fails its contract, or
    /// [`check_answer`] refuses it.
    pub fn evaluate(
        &mut self,
        request: &EvaluationRequest,
    ) -> Result<CandidateEvaluation, SandboxError> {
        let mut stream = self.connect()?;
        set_timeouts(&stream, EXCHANGE_BUDGET)?;
        stream
            .write_all(request.encode_line()?.as_bytes())
            .map_err(|error| SandboxError::io("sending a request", error))?;
        let line = match read_line(&mut stream, MAX_ANSWER_BYTES, EXCHANGE_BUDGET, "the answer")? {
            Ok(line) => line,
            Err(Unframed::Closed(bytes)) => {
                return Err(SandboxError::Protocol(format!(
                    "the guest closed the stream after {bytes} bytes of its answer"
                )));
            }
        };
        if let Some(reason) = line.strip_prefix("ERROR ") {
            return Err(SandboxError::Refused(format!(
                "the guest refused: {}",
                reason.trim_end()
            )));
        }
        let answer = CandidateEvaluation::decode(line.trim_end_matches('\n'))?;
        check_answer(request, &answer)?;
        Ok(answer)
    }

    /// Connects to the guest's evaluation port through the hybrid vsock.
    fn connect(&mut self) -> Result<UnixStream, SandboxError> {
        for _ in 0..MAX_POLLS {
            if self.started.elapsed() > BOOT_BUDGET {
                return Err(deadline_error(
                    "the guest accepting on its vsock port",
                    BOOT_BUDGET,
                ));
            }
            if !self.alive()? {
                return Err(SandboxError::Refused(format!(
                    "firecracker for sandbox {} exited before its guest accepted; see {}",
                    self.vm_id.get(),
                    self.dir.join("console.log").display()
                )));
            }
            if let Some(stream) = self.handshake()? {
                return Ok(stream);
            }
            std::thread::sleep(POLL);
        }
        Err(deadline_error(
            "the guest accepting on its vsock port",
            BOOT_BUDGET,
        ))
    }

    /// One handshake attempt; `None` means "not listening yet".
    fn handshake(&self) -> Result<Option<UnixStream>, SandboxError> {
        let mut stream = match UnixStream::connect(&self.socket) {
            Ok(stream) => stream,
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::NotFound | ErrorKind::ConnectionRefused
                ) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(SandboxError::io("connecting to the vsock socket", error)),
        };
        set_timeouts(&stream, HANDSHAKE_BUDGET)?;
        stream
            .write_all(connect_command(EVALUATION_PORT).as_bytes())
            .map_err(|error| SandboxError::io("sending the vsock connect command", error))?;
        match read_line(
            &mut stream,
            MAX_ACK_BYTES,
            HANDSHAKE_BUDGET,
            "the vsock handshake",
        )? {
            Ok(line) => {
                parse_ack(&line)?;
                Ok(Some(stream))
            }
            Err(Unframed::Closed(_)) => Ok(None),
        }
    }

    /// Kills the process, reaps it within [`STOP_BUDGET`] and removes its
    /// socket files.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxError::Deadline`] when the process is not reaped in
    /// time, and [`SandboxError::Io`] when it cannot be signalled or a socket
    /// cannot be removed.
    pub fn stop(mut self) -> Result<Stopped, SandboxError> {
        let status = self.reap()?;
        let sockets_removed = remove_sockets(&self.dir)?;
        Ok(Stopped {
            vm_id: self.vm_id,
            status,
            sockets_removed,
        })
    }

    /// Kills and reaps the process, bounded.
    fn reap(&mut self) -> Result<ExitStatus, SandboxError> {
        match self.child.kill() {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::InvalidInput => {}
            Err(error) => return Err(SandboxError::io("killing a firecracker process", error)),
        }
        let started = Instant::now();
        for _ in 0..MAX_POLLS {
            let polled = self
                .child
                .try_wait()
                .map_err(|error| SandboxError::io("reaping a firecracker process", error))?;
            if let Some(status) = polled {
                self.reaped = true;
                return Ok(status);
            }
            if started.elapsed() > STOP_BUDGET {
                break;
            }
            std::thread::sleep(POLL);
        }
        Err(deadline_error("reaping a firecracker process", STOP_BUDGET))
    }
}

impl Drop for RunningVm {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.reap();
        }
    }
}

/// Starts Firecracker with the configuration file and nothing else.
fn spawn(
    firecracker: &Path,
    config: &Path,
    dir: &Path,
    vm_id: VmId,
) -> Result<Child, SandboxError> {
    let console = File::create(dir.join("console.log"))
        .map_err(|error| SandboxError::io("creating a console log", error))?;
    let errors = console
        .try_clone()
        .map_err(|error| SandboxError::io("sharing a console log", error))?;
    Command::new(firecracker)
        .arg("--no-api")
        .arg("--config-file")
        .arg(config)
        .arg("--id")
        .arg(format!("aegis-m21-vm-{}", vm_id.get()))
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(console)
        .stderr(errors)
        .spawn()
        .map_err(|error| SandboxError::io("starting firecracker", error))
}

/// Applies the same read and write timeout to `stream`.
fn set_timeouts(stream: &UnixStream, budget: Duration) -> Result<(), SandboxError> {
    stream
        .set_read_timeout(Some(budget))
        .and_then(|()| stream.set_write_timeout(Some(budget)))
        .map_err(|error| SandboxError::io("setting a socket timeout", error))
}

/// Removes every Unix socket file in `dir`, bounded, and returns the count.
fn remove_sockets(dir: &Path) -> Result<usize, SandboxError> {
    use std::os::unix::fs::FileTypeExt;
    let entries = fs::read_dir(dir)
        .map_err(|error| SandboxError::io("listing a microVM directory", error))?;
    let mut removed = 0_usize;
    for entry in entries.take(MAX_DIRECTORY_ENTRIES) {
        let entry =
            entry.map_err(|error| SandboxError::io("listing a microVM directory", error))?;
        let kind = entry
            .file_type()
            .map_err(|error| SandboxError::io("reading a file type", error))?;
        if kind.is_socket() {
            fs::remove_file(entry.path())
                .map_err(|error| SandboxError::io("removing a vsock socket", error))?;
            removed = removed.saturating_add(1);
        }
    }
    Ok(removed)
}

/// Returns the guest memory every microVM of the reference run is given.
///
/// # Errors
///
/// Propagates [`GuestMemoryMib::new`].
pub fn reference_memory() -> Result<GuestMemoryMib, SandboxError> {
    Ok(GuestMemoryMib::new(crate::REFERENCE_GUEST_MEMORY_MIB)?)
}

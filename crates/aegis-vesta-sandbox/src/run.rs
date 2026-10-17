// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The host run `make verify-workstation` starts: E21-2 on real KVM.
//!
//! In order, one microVM at a time:
//!
//! 1. negative: a request above the guest memory limit is refused by
//!    [`GuestMemoryMib::new`] before any row or process exists;
//! 2. positive: the first admitted microVM boots and two candidate
//!    evaluations round-trip over `AF_VSOCK`, one the Pareto gate passes and
//!    one at the 1.5 ms latency bound it fails -- the verdicts are the guest's;
//! 3. M22 / E22-2: the same microVM answers
//!    [`ROUND_TRIPS`](crate::transport::ROUND_TRIPS) more evaluations,
//!    timed into a [`RoundTripFigure`] that names Firecracker, with the host's
//!    vhost state read before the first boot and while it runs;
//! 4. boundary: microVMs 2 to 64 are admitted, booted and each answers one
//!    evaluation, so the 64th is accepted with all 64 processes running;
//! 5. M22 / E22-1: each of those processes' resident set after its first
//!    answer becomes a [`FootprintFigure`] that names the monitor the process
//!    was found to run;
//! 6. boundary: the 65th request is refused by the controller and no process
//!    is started for it;
//! 7. every row is terminated, every process killed and reaped, and every
//!    socket file removed.
//!
//! Each step prints `PASS <case>` or `FAIL <case>` with detail lines. A step
//! that cannot run because an earlier one failed is a failure, not a skip.

use core::fmt::Display;
use std::io::Write;

use aegis_vesta::{
    CorrelationId, GuestMemoryMib, Label, MAX_GUEST_MEMORY_MIB, MAX_MICROVMS, MicroVmController,
    MicroVmRequest, VestaError, VmId, VmmIdentity,
};

use crate::boot::{FootprintFigure, FootprintSample, measure};
use crate::error::SandboxError;
use crate::request::{EvaluationRequest, RequestVersion, WireMetrics};
use crate::transport::{HostVsock, RoundTripFigure, check_mechanism, micros, time_round_trips};
use crate::vm::{Launch, RunningVm, Stopped, evaluate_in, reference_memory};

/// Metrics every Pareto objective clears: the positive candidate.
pub const SUPERIOR: WireMetrics = WireMetrics {
    latency_ms: 1.2,
    memory_mb: 32.0,
    carbon_rate: 0.5,
    retention: 0.995,
};

/// Metrics exactly at the 1.5 ms latency bound, which the strict comparison
/// fails: the candidate a guest that echoed "passed" would get wrong.
pub const AT_LATENCY_BOUND: WireMetrics = WireMetrics {
    latency_ms: 1.5,
    memory_mb: 32.0,
    carbon_rate: 0.5,
    retention: 0.995,
};

/// The cases the host run prints, in order.
pub const HOST_CASES: [&str; 7] = [
    "sandbox/over-limit-memory-refused",
    "sandbox/boot-and-round-trip",
    "sandbox/vsock-transport",
    "sandbox/sixty-fourth-accepted",
    "sandbox/footprint",
    "sandbox/sixty-fifth-refused",
    "sandbox/teardown",
];

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Every case passed.
    Passed,
    /// This many cases failed.
    Failed(u32),
}

/// One footprint measured per started microVM, or why it could not be.
type Boots = Vec<Result<FootprintSample, String>>;

/// The report the run writes.
struct Report<'w> {
    out: &'w mut dyn Write,
    failed: u32,
}

impl Report<'_> {
    fn info(&mut self, key: &str, value: impl Display) -> Result<(), SandboxError> {
        writeln!(self.out, "info {key}: {value}")
            .map_err(|error| SandboxError::io("writing the report", error))
    }

    fn case(
        &mut self,
        name: &str,
        result: Result<Vec<String>, SandboxError>,
    ) -> Result<bool, SandboxError> {
        let (verdict, notes, passed) = match result {
            Ok(notes) => ("PASS", notes, true),
            Err(error) => ("FAIL", vec![error.to_string()], false),
        };
        if !passed {
            self.failed = self.failed.saturating_add(1);
        }
        writeln!(self.out, "{verdict} {name}")
            .map_err(|error| SandboxError::io("writing the report", error))?;
        for note in notes {
            writeln!(self.out, "     {note}")
                .map_err(|error| SandboxError::io("writing the report", error))?;
        }
        Ok(passed)
    }
}

/// Builds the admission request for the `ordinal`-th microVM.
///
/// # Errors
///
/// Returns [`SandboxError::Id`] when the label passes its bound, and
/// [`SandboxError::Vesta`] when the request is refused.
pub fn admission(ordinal: usize, memory: GuestMemoryMib) -> Result<MicroVmRequest, SandboxError> {
    let name = Label::parse(&format!("aegis-m21-microvm-{ordinal:02}"))?;
    Ok(MicroVmRequest::new(
        name,
        memory,
        false,
        VmmIdentity::Firecracker,
    )?)
}

/// Builds an evaluation request for sandbox `vm_id`.
///
/// # Errors
///
/// Returns [`SandboxError::Id`] when an identifier passes its bound.
pub fn request(
    vm_id: VmId,
    tag: &str,
    metrics: WireMetrics,
) -> Result<EvaluationRequest, SandboxError> {
    Ok(EvaluationRequest {
        schema: RequestVersion::V1,
        correlation_id: CorrelationId::parse(&format!("aegis-m21-vm{}-{tag}", vm_id.get()))?,
        candidate: Label::parse(&format!("m21-candidate-{tag}"))?,
        vm_id,
        vmm: VmmIdentity::Firecracker,
        metrics,
    })
}

/// Case 1: an over-limit memory request never becomes a value.
///
/// # Errors
///
/// Returns [`SandboxError::Refused`] when the request is not refused or a
/// process already exists.
pub fn over_limit_memory(fleet: &[RunningVm]) -> Result<Vec<String>, SandboxError> {
    let over = MAX_GUEST_MEMORY_MIB.saturating_add(1);
    let refused = GuestMemoryMib::new(over);
    let Err(VestaError::GuestMemoryOutOfRange { .. }) = refused else {
        return Err(SandboxError::Refused(format!(
            "{over} MiB was not refused: {refused:?}"
        )));
    };
    let at_limit = GuestMemoryMib::new(MAX_GUEST_MEMORY_MIB)?;
    if !fleet.is_empty() {
        return Err(SandboxError::Refused(
            "a process exists before the first admission".to_owned(),
        ));
    }
    Ok(vec![
        format!(
            "{over} MiB refused before any row or process: {}",
            VestaError::GuestMemoryOutOfRange {
                mib: over,
                min: aegis_vesta::MIN_GUEST_MEMORY_MIB,
                max: MAX_GUEST_MEMORY_MIB,
            }
        ),
        format!(
            "{} MiB, the limit itself, is admissible as a request",
            at_limit.get()
        ),
        "no firecracker process was started for either".to_owned(),
    ])
}

/// Admits the next microVM, starts it, has it answer once and measures its
/// footprint into `boots`.
fn admit_and_boot(
    controller: &mut MicroVmController,
    launch: &Launch<'_>,
    fleet: &mut Vec<RunningVm>,
    boots: &mut Boots,
    memory: GuestMemoryMib,
) -> Result<VmId, SandboxError> {
    let ordinal = fleet.len().saturating_add(1);
    let vm_id = controller.spawn(admission(ordinal, memory)?)?;
    let instance = controller.get(vm_id).ok_or_else(|| {
        SandboxError::Refused(format!("the controller holds no row for {}", vm_id.get()))
    })?;
    fleet.push(RunningVm::start(launch, &instance)?);
    let vm = fleet
        .last_mut()
        .ok_or_else(|| SandboxError::Refused("the fleet is empty after a start".to_owned()))?;
    evaluate_in(vm, &request(vm_id, "superior", SUPERIOR)?)?;
    boots.push(measure(vm, launch.firecracker).map_err(|error| error.to_string()));
    Ok(vm_id)
}

/// Case 2: the first microVM boots and two evaluations round-trip.
fn boot_and_round_trip(
    controller: &mut MicroVmController,
    launch: &Launch<'_>,
    fleet: &mut Vec<RunningVm>,
    boots: &mut Boots,
    memory: GuestMemoryMib,
) -> Result<Vec<String>, SandboxError> {
    let vm_id = admit_and_boot(controller, launch, fleet, boots, memory)?;
    let vm = fleet
        .last_mut()
        .ok_or_else(|| SandboxError::Refused("the fleet is empty after a start".to_owned()))?;
    let bound = evaluate_in(vm, &request(vm_id, "latency-bound", AT_LATENCY_BOUND)?)?;
    let superior = evaluate_in(vm, &request(vm_id, "superior-again", SUPERIOR)?)?;
    Ok(vec![
        format!(
            "sandbox {} (vsock context {}) booted under firecracker, {} MiB, and accepted on port {}",
            vm_id.get(),
            vm.cid().get(),
            memory.get(),
            crate::EVALUATION_PORT
        ),
        format!(
            "candidate m21-candidate-superior: verdict {} (evaluated in the guest)",
            superior.verdict.tag()
        ),
        format!(
            "candidate m21-candidate-latency-bound (latency 1.5 ms, strict bound): verdict {}",
            bound.verdict.tag()
        ),
        format!(
            "answers decoded as aegis.p10-p16.candidate-evaluation.v1; guest clock stamped {}",
            bound.evaluated_at
        ),
    ])
}

/// Case 3: the first microVM's transport, timed and its mechanism held.
fn vsock_transport(
    fleet: &mut [RunningVm],
    before: Result<HostVsock, SandboxError>,
) -> Result<Vec<String>, SandboxError> {
    let before = before?;
    let vm = fleet
        .first_mut()
        .ok_or_else(|| SandboxError::Refused("no microVM is running".to_owned()))?;
    let vm_id = vm.vm_id();
    let samples = time_round_trips(vm, vm_id)?;
    let during = HostVsock::read()?;
    check_mechanism(VmmIdentity::Firecracker, before, during)?;
    let figure = RoundTripFigure::from_samples(
        VmmIdentity::Firecracker,
        VmmIdentity::Firecracker,
        &samples,
    )?;
    let count = |host: HostVsock| host.references.map_or("none".to_owned(), |n| n.to_string());
    Ok(vec![
        format!(
            "host: {} {}, {}; vhost_vsock references {} before the first boot, {} while it ran",
            crate::transport::VHOST_DEVICE,
            if during.device { "present" } else { "absent" },
            during.module.config_line(),
            count(before),
            count(during)
        ),
        "mechanism: Firecracker's own virtio-vsock device, AF_UNIX on the host, AF_VSOCK in \
         the guest; vhost not in the path"
            .to_owned(),
        format!(
            "vmm {} {}: {} round trips on sandbox {}, min {}, median {}, max {}",
            figure.vmm.tag(),
            figure.version,
            figure.samples,
            vm_id.get(),
            micros(figure.min),
            micros(figure.median),
            micros(figure.max)
        ),
        format!(
            "samples: {}",
            samples
                .iter()
                .map(|d| micros(*d))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ])
}

/// Case 4: microVMs up to [`MAX_MICROVMS`] are admitted, booted and answer,
/// and all of them are running at once.
fn fill_to_the_bound(
    controller: &mut MicroVmController,
    launch: &Launch<'_>,
    fleet: &mut Vec<RunningVm>,
    boots: &mut Boots,
    memory: GuestMemoryMib,
) -> Result<Vec<String>, SandboxError> {
    for _ in fleet.len()..MAX_MICROVMS {
        admit_and_boot(controller, launch, fleet, boots, memory)?;
    }
    let mut running = 0_usize;
    for vm in fleet.iter_mut().take(MAX_MICROVMS) {
        if vm.alive()? {
            running = running.saturating_add(1);
        }
    }
    let last = fleet.last().map_or(0, |vm| vm.vm_id().get());
    if controller.count() != MAX_MICROVMS || running != MAX_MICROVMS {
        return Err(SandboxError::Refused(format!(
            "the controller holds {} rows and {running} processes are running; {MAX_MICROVMS} expected",
            controller.count()
        )));
    }
    Ok(vec![
        format!(
            "the {MAX_MICROVMS}th request was admitted as sandbox {last}, booted on KVM and answered"
        ),
        format!(
            "{running} firecracker processes running at once, one per admitted row, {} MiB each",
            memory.get()
        ),
    ])
}

/// Case 5: every footprint of the fill becomes one figure naming its
/// monitor.
///
/// # Errors
///
/// Returns [`SandboxError::Refused`] for the first microVM that could not be
/// measured, and propagates [`FootprintFigure::from_samples`].
pub fn footprint(boots: &[Result<FootprintSample, String>]) -> Result<Vec<String>, SandboxError> {
    let mut samples = Vec::with_capacity(boots.len());
    for (ordinal, boot) in boots.iter().enumerate().take(MAX_MICROVMS) {
        let sample = boot.as_ref().map_err(|reason| {
            SandboxError::Refused(format!(
                "microVM {} was not measured: {reason}",
                ordinal.saturating_add(1)
            ))
        })?;
        samples.push(*sample);
    }
    let figure = FootprintFigure::from_samples(VmmIdentity::Firecracker, &samples)?;
    let (min, median, max) = figure.resident_kib;
    Ok(vec![
        format!(
            "vmm {} {}: {} processes; each one's /proc/<pid>/exe is the pinned binary",
            figure.vmm.tag(),
            figure.version,
            figure.samples
        ),
        format!(
            "monitor resident set (VmRSS) after the first answer: min {min} KiB, median \
             {median} KiB, max {max} KiB"
        ),
        "boot time: not measured; the guest's boot timer write waits on an open decision"
            .to_owned(),
    ])
}

/// Case 6: the 65th request is refused and no process is started for it.
///
/// # Errors
///
/// Returns [`SandboxError::Refused`] when the request is admitted, or a row
/// or a process appears for it.
pub fn sixty_fifth(
    controller: &mut MicroVmController,
    fleet: &[RunningVm],
    memory: GuestMemoryMib,
) -> Result<Vec<String>, SandboxError> {
    let ordinal = MAX_MICROVMS.saturating_add(1);
    let before = fleet.len();
    let refused = controller.spawn(admission(ordinal, memory)?);
    let Err(error @ VestaError::MicroVmTableFull { .. }) = refused else {
        return Err(SandboxError::Refused(format!(
            "request {ordinal} was not refused: {refused:?}"
        )));
    };
    if fleet.len() != before || controller.count() != MAX_MICROVMS {
        return Err(SandboxError::Refused(
            "a row or a process appeared for the refused request".to_owned(),
        ));
    }
    Ok(vec![
        format!("request {ordinal} refused by the controller: {error}"),
        format!("still {before} processes; none was started for it"),
    ])
}

/// Case 7: every row terminated, every process reaped, every socket removed.
fn teardown(
    controller: &mut MicroVmController,
    fleet: &mut Vec<RunningVm>,
) -> Result<Vec<String>, SandboxError> {
    let mut stopped: Vec<Stopped> = Vec::with_capacity(fleet.len());
    let mut first_error = None;
    for vm in fleet.drain(..).take(MAX_MICROVMS) {
        let vm_id = vm.vm_id();
        if !controller.terminate(vm_id) {
            first_error.get_or_insert(SandboxError::Refused(format!(
                "the controller does not hold {}",
                vm_id.get()
            )));
        }
        match vm.stop() {
            Ok(done) => stopped.push(done),
            Err(error) => {
                first_error.get_or_insert(SandboxError::from(error));
            }
        }
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    let sockets: usize = stopped.iter().map(|done| done.sockets_removed).sum();
    if controller.running() != 0 {
        return Err(SandboxError::Refused(format!(
            "{} rows still running",
            controller.running()
        )));
    }
    Ok(vec![
        format!(
            "{} rows terminated; the controller reports 0 running",
            stopped.len()
        ),
        format!("{} firecracker processes killed and reaped", stopped.len()),
        format!("{sockets} vsock socket files removed; no network device was configured"),
    ])
}

/// Runs the cases that need the first microVM's round trip: the fill to the
/// bound, its footprint and the 65th refusal.
fn bound_cases(
    report: &mut Report<'_>,
    controller: &mut MicroVmController,
    launch: &Launch<'_>,
    fleet: &mut Vec<RunningVm>,
    boots: &mut Boots,
    booted: bool,
) -> Result<(), SandboxError> {
    let memory = reference_memory()?;
    let filled = if booted {
        fill_to_the_bound(controller, launch, fleet, boots, memory)
    } else {
        Err(not_run("the first microVM did not round-trip"))
    };
    let filled = report.case("sandbox/sixty-fourth-accepted", filled)?;
    let measured = if filled {
        footprint(boots)
    } else {
        Err(not_run("the table was not filled to its bound"))
    };
    report.case("sandbox/footprint", measured)?;
    let refusal = if filled {
        sixty_fifth(controller, fleet, memory)
    } else {
        Err(not_run("the table was not filled to its bound"))
    };
    report.case("sandbox/sixty-fifth-refused", refusal)?;
    Ok(())
}

/// The failure of a case an earlier failure kept from running.
fn not_run(why: &str) -> SandboxError {
    SandboxError::Refused(format!("not run: {why}"))
}

/// Runs the seven cases against the pinned binary, kernel and initramfs.
///
/// # Errors
///
/// Returns [`SandboxError`] only when the report cannot be written; every
/// other failure is a failed case.
pub fn run_sandbox(launch: &Launch<'_>, out: &mut dyn Write) -> Result<Verdict, SandboxError> {
    let mut report = Report { out, failed: 0 };
    let memory = reference_memory()?;
    report.info(
        "guest memory",
        format!("{} MiB per microVM, 1 vCPU", memory.get()),
    )?;
    report.info("boot args", crate::BOOT_ARGS)?;
    report.info("controller bound", MAX_MICROVMS)?;
    let host_before = HostVsock::read();
    let mut controller = MicroVmController::new();
    let mut fleet: Vec<RunningVm> = Vec::with_capacity(MAX_MICROVMS);
    let mut boots: Boots = Vec::with_capacity(MAX_MICROVMS);
    report.case(
        "sandbox/over-limit-memory-refused",
        over_limit_memory(&fleet),
    )?;
    let booted = report.case(
        "sandbox/boot-and-round-trip",
        boot_and_round_trip(&mut controller, launch, &mut fleet, &mut boots, memory),
    )?;
    let timed = if booted {
        vsock_transport(&mut fleet, host_before)
    } else {
        Err(SandboxError::Refused(
            "not run: the first microVM did not round-trip".to_owned(),
        ))
    };
    report.case("sandbox/vsock-transport", timed)?;
    bound_cases(
        &mut report,
        &mut controller,
        launch,
        &mut fleet,
        &mut boots,
        booted,
    )?;
    report.case("sandbox/teardown", teardown(&mut controller, &mut fleet))?;
    Ok(if report.failed == 0 {
        Verdict::Passed
    } else {
        Verdict::Failed(report.failed)
    })
}

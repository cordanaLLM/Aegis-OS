// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E21-2's admission half, through the functions the real run calls.
//!
//! `make verify-workstation` boots a microVM only for a row the
//! `aegis-vesta` controller admitted, so the 64/65 boundary and the memory
//! refusal are decided here, without KVM, by exactly the code the run uses.
//! Positive: 64 admissions are accepted and each row's context identifier is
//! the one the guest reads back. Negative: a request above the 1024 MiB limit
//! never becomes a value. Boundary: the 65th request is refused with the
//! table full, and with one row free the same check refuses to call it the
//! 65th.

mod common;

use aegis_vesta::{
    GuestMemoryMib, MAX_GUEST_MEMORY_MIB, MAX_MICROVMS, MIN_GUEST_MEMORY_MIB, MicroVmController,
    VestaError, VmmIdentity,
};
use aegis_vesta_sandbox::{
    REFERENCE_GUEST_MEMORY_MIB, SandboxError, admission, over_limit_memory, reference_memory,
    sixty_fifth, vm_id_for_cid,
};

use common::{FIRST_CID, FIRST_VM, Fallible};

/// Fills a controller with `count` admissions of the reference memory.
fn filled(count: usize) -> Result<MicroVmController, Box<dyn std::error::Error>> {
    let mut controller = MicroVmController::new();
    let memory = reference_memory()?;
    for ordinal in 1..=count {
        controller.spawn(admission(ordinal, memory)?)?;
    }
    Ok(controller)
}

// --- Positive -------------------------------------------------------------

/// Positive: sixty-four admissions are accepted, each under Firecracker, and
/// the context identifier of every row maps back to its sandbox.
#[test]
fn sixty_four_admissions_are_accepted() -> Fallible {
    let controller = filled(MAX_MICROVMS)?;
    assert_eq!(controller.count(), MAX_MICROVMS);
    assert_eq!(controller.running(), MAX_MICROVMS);
    for raw in FIRST_VM..FIRST_VM.saturating_add(u32::try_from(MAX_MICROVMS)?) {
        let row = controller
            .get(aegis_vesta::VmId::new(raw))
            .ok_or("an admitted row")?;
        assert_eq!(row.vmm, VmmIdentity::Firecracker);
        assert_eq!(row.memory.get(), REFERENCE_GUEST_MEMORY_MIB);
        assert!(!row.accelerator);
        assert_eq!(vm_id_for_cid(row.vsock_cid.get())?, row.vm_id);
    }
    Ok(())
}

/// Positive: the first row's context identifier is the one the run prints.
#[test]
fn the_first_row_is_sandbox_one_hundred() -> Fallible {
    let controller = filled(1)?;
    let row = controller
        .get(aegis_vesta::VmId::new(FIRST_VM))
        .ok_or("the first row")?;
    assert_eq!(row.vsock_cid.get(), FIRST_CID);
    assert_eq!(row.name.to_string(), "aegis-m21-microvm-01");
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative (E21-2): a request above the memory limit is refused before any
/// row or process exists, and the case the run prints says so.
#[test]
fn an_over_limit_memory_request_is_refused() -> Fallible {
    let over = MAX_GUEST_MEMORY_MIB.saturating_add(1);
    assert!(matches!(
        GuestMemoryMib::new(over),
        Err(VestaError::GuestMemoryOutOfRange {
            mib: 1025,
            max: 1024,
            ..
        })
    ));
    let notes = over_limit_memory(&[])?;
    assert!(notes.iter().any(|note| note.contains("1025 MiB refused")));
    assert!(
        notes
            .iter()
            .any(|note| note.contains("no firecracker process"))
    );
    Ok(())
}

/// Negative: a reserved context identifier names no sandbox.
#[test]
fn a_reserved_context_identifier_is_refused() {
    for reserved in [0, 1, 2] {
        assert!(matches!(
            vm_id_for_cid(reserved),
            Err(SandboxError::Refused(_))
        ));
    }
}

// --- Boundary -------------------------------------------------------------

/// Boundary (E21-2): with the table full the 65th request is refused, and
/// the check the run applies reports no process for it.
#[test]
fn the_sixty_fifth_request_is_refused() -> Fallible {
    let mut controller = filled(MAX_MICROVMS)?;
    let memory = reference_memory()?;
    let notes = sixty_fifth(&mut controller, &[], memory)?;
    assert!(notes.iter().any(|note| note.contains("request 65 refused")));
    assert_eq!(controller.count(), MAX_MICROVMS);
    Ok(())
}

/// Boundary: one row short of the bound, the next request is admitted, so
/// the same check refuses to report a refusal.
#[test]
fn one_row_short_the_next_is_admitted() -> Fallible {
    let short = MAX_MICROVMS.checked_sub(1).ok_or("a bound above zero")?;
    let mut controller = filled(short)?;
    let memory = reference_memory()?;
    assert!(matches!(
        sixty_fifth(&mut controller, &[], memory),
        Err(SandboxError::Refused(_))
    ));
    assert_eq!(controller.count(), MAX_MICROVMS);
    Ok(())
}

/// Boundary: the guest memory range is closed at both ends, and the reference
/// size sits inside it.
#[test]
fn the_memory_range_is_closed_at_both_ends() -> Fallible {
    assert!(GuestMemoryMib::new(MIN_GUEST_MEMORY_MIB).is_ok());
    assert!(GuestMemoryMib::new(MAX_GUEST_MEMORY_MIB).is_ok());
    assert!(GuestMemoryMib::new(0).is_err());
    assert_eq!(reference_memory()?.get(), 64);
    Ok(())
}

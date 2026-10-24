// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Halt-latch refusal integration tests for [`SignedAuditRecord`].
//!
//! A record of the halt latch itself (`HaltEngaged` or `HaltReleased`) decides
//! no action and names no subject, so the action-decision schema refuses to
//! encode it.

mod common;

use aegis_justitia::{BlockReason, ContractError, HaltReason, PayloadBuffer, RecordStatus};
use common::{Fallible, audit_record};

// --- Positive -------------------------------------------------------------

/// Positive: a normal decision record encodes without error.
#[test]
fn positive_normal_record_encodes() -> Fallible<()> {
    let record = audit_record(1)?;
    assert_eq!(record.status, RecordStatus::Approved);
    assert_eq!(record.validate(), Ok(()));
    let mut buffer = PayloadBuffer::new();
    let text = record.encode_into(&mut buffer)?;
    assert_ne!(text, "");
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a record with status `HaltEngaged` is refused with `ContractError::Malformed`.
#[test]
fn negative_halt_engaged_record_refused() -> Fallible<()> {
    let mut record = audit_record(1)?;
    record.status = RecordStatus::HaltEngaged;
    assert_eq!(
        record.validate(),
        Err(ContractError::Malformed {
            correlation: record.correlation()
        })
    );
    let mut buffer = PayloadBuffer::new();
    assert_eq!(
        record.encode_into(&mut buffer),
        Err(ContractError::Malformed {
            correlation: record.correlation()
        })
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: `HaltReleased` is refused with `Malformed`, while `Halted` (engine refusal)
/// is accepted when a reason is present, and refused with `RefusalWithoutReason` when absent.
#[test]
fn boundary_halt_released_and_halted_behaviour() -> Fallible<()> {
    // HaltReleased is refused as Malformed
    let mut released = audit_record(1)?;
    released.status = RecordStatus::HaltReleased;
    assert_eq!(
        released.validate(),
        Err(ContractError::Malformed {
            correlation: released.correlation()
        })
    );
    let mut buffer = PayloadBuffer::new();
    assert_eq!(
        released.encode_into(&mut buffer),
        Err(ContractError::Malformed {
            correlation: released.correlation()
        })
    );

    // Halted with BlockReason encodes successfully
    let mut halted = audit_record(1)?;
    halted.status = RecordStatus::Halted;
    halted.block_reason = Some(BlockReason::KillswitchEngaged(HaltReason::OperatorStop));
    assert_eq!(halted.validate(), Ok(()));
    let mut buffer = PayloadBuffer::new();
    let text = halted.encode_into(&mut buffer)?;
    assert_ne!(text, "");

    // Halted without BlockReason is refused with RefusalWithoutReason
    let mut halted_no_reason = audit_record(1)?;
    halted_no_reason.status = RecordStatus::Halted;
    halted_no_reason.block_reason = None;
    assert_eq!(
        halted_no_reason.validate(),
        Err(ContractError::RefusalWithoutReason {
            correlation: halted_no_reason.correlation()
        })
    );
    let mut buffer = PayloadBuffer::new();
    assert_eq!(
        halted_no_reason.encode_into(&mut buffer),
        Err(ContractError::RefusalWithoutReason {
            correlation: halted_no_reason.correlation()
        })
    );
    Ok(())
}

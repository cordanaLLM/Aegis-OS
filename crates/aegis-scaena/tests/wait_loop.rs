// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `dispatch_until` on its own, against a Wayland peer that never answers:
//! the wait ends on the caller's predicate or on the caller's deadline, and a
//! failure names the event the caller waited for (HISS-02).
//!
//! Positive: a predicate that holds ends the wait at once. Negative: a
//! predicate that never holds fails at the deadline, and a peer that hangs up
//! fails at once; both name the awaited event. Boundary: a zero deadline
//! fails a predicate that does not hold without waiting and still lets one
//! that holds pass, since the predicate is checked before the deadline.

mod common;

use core::time::Duration;
use std::os::unix::net::UnixStream;
use std::time::Instant;

use aegis_scaena::smithay_client_toolkit::reexports::client::Connection;
use aegis_scaena::{Wait, WaitError, WaitEvent, dispatch_until};

use common::Fallible;

/// How far past its deadline a failure may land on a loaded machine.
const SLACK: Duration = Duration::from_secs(5);

/// The deadline of a wait that is meant to run out.
const SHORT: Duration = Duration::from_millis(200);

/// The deadline of a wait that is meant to end before it.
const LONG: Duration = Duration::from_secs(30);

/// The event every wait here names; not the registry roundtrip, so a failure
/// that names another event than the caller's is caught.
const EVENT: WaitEvent = WaitEvent::Presented;

/// Runs one wait on a fresh connection whose peer stays silent, or hangs up
/// first when `hang_up` is set; returns the outcome and how long it took.
fn wait_on_silent_peer(
    deadline: Duration,
    hang_up: bool,
    done: bool,
) -> Result<(Result<(), WaitError>, Duration), Box<dyn std::error::Error>> {
    let (client, server) = UnixStream::pair()?;
    let connection = Connection::from_socket(client)?;
    let mut queue = connection.new_event_queue::<u32>();
    let mut state = 0_u32;
    let held = if hang_up {
        drop(server);
        None
    } else {
        Some(server)
    };
    let wait = Wait {
        event: EVENT,
        deadline,
    };
    let started = Instant::now();
    let outcome = dispatch_until(&mut queue, &mut state, wait, |_| done);
    let elapsed = started.elapsed();
    drop(held);
    Ok((outcome, elapsed))
}

// --- Positive -------------------------------------------------------------

/// Positive: a predicate that holds ends the wait after its first dispatch,
/// long before the deadline, though the peer sends nothing.
#[test]
fn a_predicate_that_holds_ends_the_wait_at_once() -> Fallible {
    let (outcome, elapsed) = wait_on_silent_peer(LONG, false, true)?;
    assert_eq!(outcome, Ok(()));
    assert!(elapsed < SLACK, "the wait took {elapsed:?}");
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a predicate that never holds fails at the deadline, not before,
/// naming the event the caller waited for.
#[test]
fn a_predicate_that_never_holds_fails_at_the_deadline() -> Fallible {
    let (outcome, elapsed) = wait_on_silent_peer(SHORT, false, false)?;
    assert_eq!(outcome, Err(WaitError::Deadline { event: EVENT }));
    assert!(elapsed >= SHORT, "failed early, after {elapsed:?}");
    assert!(
        elapsed < SHORT.saturating_add(SLACK),
        "failed late, after {elapsed:?}"
    );
    Ok(())
}

/// Negative: a peer that hangs up fails the wait at once as a connection
/// failure naming the awaited event.
#[test]
fn a_peer_that_hangs_up_fails_the_wait_at_once() -> Fallible {
    let (outcome, elapsed) = wait_on_silent_peer(LONG, true, false)?;
    assert_eq!(outcome, Err(WaitError::Connection { event: EVENT }));
    assert!(elapsed < SLACK, "the failure took {elapsed:?}");
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: with a zero deadline a predicate that does not hold fails
/// without waiting, and one that holds still passes.
#[test]
fn a_zero_deadline_still_checks_the_predicate_first() -> Fallible {
    let (missed, elapsed) = wait_on_silent_peer(Duration::ZERO, false, false)?;
    assert_eq!(missed, Err(WaitError::Deadline { event: EVENT }));
    assert!(elapsed < SLACK, "the failure took {elapsed:?}");
    let (held, _) = wait_on_silent_peer(Duration::ZERO, false, true)?;
    assert_eq!(held, Ok(()));
    Ok(())
}

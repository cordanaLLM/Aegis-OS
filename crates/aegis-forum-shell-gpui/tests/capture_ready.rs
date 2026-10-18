// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Tests for the bounded readiness poll that gates a ring capture (M28 E28-4).
//!
//! The probe is a closure, so the loop is checked without a compositor or bus.

use std::cell::Cell;
use std::time::Duration;

use aegis_forum_shell_gpui::capture::{CaptureError, poll_ready};

const TICK: Duration = Duration::from_millis(1);

#[test]
fn ready_on_first_probe_returns_after_one_call() {
    let calls = Cell::new(0_u32);
    let got = async_io::block_on(poll_ready(5, TICK, || {
        calls.set(calls.get().saturating_add(1));
        async { Ok(true) }
    }));
    assert!(matches!(got, Ok(true)));
    assert_eq!(calls.get(), 1);
}

#[test]
fn never_ready_stops_at_attempt_bound() {
    let calls = Cell::new(0_u32);
    let got = async_io::block_on(poll_ready(4, TICK, || {
        calls.set(calls.get().saturating_add(1));
        async { Ok(false) }
    }));
    assert!(matches!(got, Ok(false)));
    assert_eq!(calls.get(), 4);
}

#[test]
fn ready_on_last_allowed_probe_counts() {
    let calls = Cell::new(0_u32);
    let got = async_io::block_on(poll_ready(3, TICK, || {
        let n = calls.get().saturating_add(1);
        calls.set(n);
        async move { Ok(n == 3) }
    }));
    assert!(matches!(got, Ok(true)));
    assert_eq!(calls.get(), 3);
}

#[test]
fn zero_attempts_probes_nothing() {
    let calls = Cell::new(0_u32);
    let got = async_io::block_on(poll_ready(0, TICK, || {
        calls.set(calls.get().saturating_add(1));
        async { Ok(true) }
    }));
    assert!(matches!(got, Ok(false)));
    assert_eq!(calls.get(), 0);
}

#[test]
fn probe_error_ends_the_poll() {
    let calls = Cell::new(0_u32);
    let got = async_io::block_on(poll_ready(5, TICK, || {
        calls.set(calls.get().saturating_add(1));
        async { Err(CaptureError::Bus("gone".to_owned())) }
    }));
    assert!(matches!(got, Err(CaptureError::Bus(_))));
    assert_eq!(calls.get(), 1);
}

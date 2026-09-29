// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E27-1 boundary: every receive, every send and every Wayland wait fails at
//! its deadline instead of hanging (HISS-02).
//!
//! Positive: a Wayland peer that answers the registry roundtrip completes it
//! and its announced global is recorded. Negative: a silent peer fails the
//! receive at its deadline, a peer that stops reading fails the send at its
//! deadline, a Wayland peer that accepts the connection and never answers
//! fails the registry roundtrip at its deadline, naming the event, and a peer
//! that hangs up fails it at once. Boundary: the admitted deadline range is
//! inclusive at 1 ms and 60 s and exclusive past either end, and no failure
//! comes before its deadline.

mod common;

use core::time::Duration;
use std::io::Write as _;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::time::Instant;

use aegis_scaena::smithay_client_toolkit::reexports::client::Connection;
use aegis_scaena::transport::{MAX_DEADLINE, MIN_DEADLINE};
use aegis_scaena::{Deadlines, LineBuffer, Registry, TransportError, WaitError, WaitEvent, pair};
use rustix::net::{SendFlags, send};

use common::{DEADLINES, Fallible, NV12_SIZE, borrow, endpoints, memfd, request};

/// How far past its deadline a failure may land on a loaded machine.
const SLACK: Duration = Duration::from_secs(5);

/// The most messages the fill loop sends before the peer's queue must be full.
const MAX_FILL: u32 = 1_000_000;

/// Asserts that `elapsed` is at or after `deadline` and not far past it.
fn at_deadline(elapsed: Duration, deadline: Duration) {
    assert!(elapsed >= deadline, "failed early, after {elapsed:?}");
    assert!(
        elapsed < deadline.saturating_add(SLACK),
        "failed late, after {elapsed:?}"
    );
}

/// One Wayland wire message: object id, opcode and the argument words.
fn wire(object: u32, opcode: u16, args: &[u8]) -> Vec<u8> {
    let size = u32::try_from(args.len().saturating_add(8)).unwrap_or(u32::MAX);
    let mut message = Vec::with_capacity(args.len().saturating_add(8));
    message.extend_from_slice(&object.to_ne_bytes());
    let header = size.checked_shl(16).unwrap_or(0) | u32::from(opcode);
    message.extend_from_slice(&header.to_ne_bytes());
    message.extend_from_slice(args);
    message
}

/// A Wayland string argument: length with the terminator, bytes, padding.
fn wire_string(text: &str) -> Vec<u8> {
    let length = u32::try_from(text.len().saturating_add(1)).unwrap_or(u32::MAX);
    let mut arg = length.to_ne_bytes().to_vec();
    arg.extend_from_slice(text.as_bytes());
    arg.push(0);
    arg.resize(arg.len().next_multiple_of(4), 0);
    arg
}

// --- Positive -------------------------------------------------------------

/// Positive: a peer that announces one global and completes the sync lets
/// the roundtrip finish well inside its deadline. The client's first two
/// new ids are the registry (2) and the sync callback (3).
#[test]
fn an_answering_wayland_peer_completes_the_registry_roundtrip() -> Fallible {
    let (client, mut server) = UnixStream::pair()?;
    let mut global = 1_u32.to_ne_bytes().to_vec();
    global.extend(wire_string("zwlr_layer_shell_v1"));
    global.extend_from_slice(&5_u32.to_ne_bytes());
    server.write_all(&wire(2, 0, &global))?;
    server.write_all(&wire(3, 0, &0_u32.to_ne_bytes()))?;
    server.write_all(&wire(1, 1, &3_u32.to_ne_bytes()))?;
    let connection = Connection::from_socket(client)?;
    let registry = Registry::roundtrip(&connection, Duration::from_secs(5))?;
    let layer_shell = registry.find("zwlr_layer_shell_v1").ok_or("no global")?;
    assert_eq!((layer_shell.name, layer_shell.version), (1, 5));
    assert_eq!(registry.globals().len(), 1);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a peer that sends nothing fails the receive at its deadline.
#[test]
fn a_silent_peer_fails_the_receive_at_its_deadline() -> Fallible {
    let (_sender, receiver) = endpoints()?;
    let mut buffer = LineBuffer::new();
    let started = Instant::now();
    let refused = receiver
        .receive(&mut buffer)
        .map(|received| received.line.len());
    assert_eq!(refused, Err(TransportError::ReceiveDeadline));
    at_deadline(started.elapsed(), DEADLINES.receive);
    Ok(())
}

/// Negative: a peer that stops reading fails the send at its deadline, once
/// its queue is full.
#[test]
fn a_peer_that_stops_reading_fails_the_send_at_its_deadline() -> Fallible {
    let (sender, _receiver) = endpoints()?;
    let mut filled = false;
    for _ in 0..MAX_FILL {
        if send(sender.as_fd(), b"\n", SendFlags::DONTWAIT).is_err() {
            filled = true;
            break;
        }
    }
    assert!(filled, "the peer's queue never filled");
    let exported = memfd("scaena-send-deadline", NV12_SIZE)?;
    let mut buffer = LineBuffer::new();
    let started = Instant::now();
    let refused = sender.send_request(&request(5)?, borrow(&exported), &mut buffer);
    assert_eq!(refused, Err(TransportError::SendDeadline));
    at_deadline(started.elapsed(), DEADLINES.send);
    Ok(())
}

/// Negative: a Wayland peer that accepts the connection and never answers
/// fails the registry roundtrip at its deadline, and the failure names the
/// event it waited for.
#[test]
fn a_silent_wayland_peer_fails_the_registry_roundtrip_at_its_deadline() -> Fallible {
    let (client, _server) = UnixStream::pair()?;
    let connection = Connection::from_socket(client)?;
    let deadline = Duration::from_millis(200);
    let started = Instant::now();
    let refused =
        Registry::roundtrip(&connection, deadline).map(|registry| registry.globals().len());
    let expected = WaitError::Deadline {
        event: WaitEvent::RegistryRoundtrip,
    };
    assert_eq!(refused, Err(expected));
    at_deadline(started.elapsed(), deadline);
    assert!(expected.to_string().contains("registry roundtrip"));
    Ok(())
}

/// Negative: a Wayland peer that hangs up fails the roundtrip at once rather
/// than waiting out the deadline.
#[test]
fn a_wayland_peer_that_hangs_up_fails_at_once() -> Fallible {
    let (client, server) = UnixStream::pair()?;
    drop(server);
    let connection = Connection::from_socket(client)?;
    let started = Instant::now();
    let refused = Registry::roundtrip(&connection, Duration::from_secs(30))
        .map(|registry| registry.globals().len());
    assert_eq!(
        refused,
        Err(WaitError::Connection {
            event: WaitEvent::RegistryRoundtrip
        })
    );
    assert!(started.elapsed() < SLACK);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: 1 ms and 60 s are admitted deadlines; zero and one past 60 s
/// are refused, zero because the kernel reads it as no deadline at all.
#[test]
fn the_deadline_range_is_inclusive_at_both_ends() -> Fallible {
    for deadline in [MIN_DEADLINE, MAX_DEADLINE] {
        pair(Deadlines {
            receive: deadline,
            send: deadline,
        })?;
    }
    for deadline in [
        Duration::ZERO,
        MAX_DEADLINE.saturating_add(Duration::from_nanos(1)),
    ] {
        let refused = pair(Deadlines {
            receive: deadline,
            send: DEADLINES.send,
        })
        .map(|_| ());
        assert_eq!(refused, Err(TransportError::InvalidDeadline));
    }
    Ok(())
}

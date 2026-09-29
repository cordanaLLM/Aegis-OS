// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E27-1 boundary: after 1,000 refused messages the process holds exactly
//! the descriptors it held before them.
//!
//! This is the only test in this integration-test binary, so no concurrent
//! test opens or closes a descriptor while the count is taken. The count is
//! the number of entries in `/proc/self/fd`, read before the first message
//! and after the last. Every message carries a refusal the receiver must
//! clean up after: two fds for one descriptor, no fd, a line naming another
//! protocol version, a line naming a file-descriptor number, a line over the
//! byte bound, and a memfd the attach step refuses as not a DMA-BUF.

mod common;

use core::mem::MaybeUninit;
use std::os::fd::{AsFd, BorrowedFd};

use aegis_scaena::attach;
use aegis_scaena::line::encode_request;
use aegis_scaena::{LineBuffer, MAX_LINE_BYTES};
use rustix::io::IoSlice;
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};

use common::{Fallible, NV12_SIZE, endpoints, memfd, request};

/// How many refused messages the case sends.
const REFUSED: usize = 1_000;

/// The number of descriptors this process holds.
fn open_descriptors() -> Result<usize, Box<dyn std::error::Error>> {
    Ok(std::fs::read_dir("/proc/self/fd")?.take(65_536).count())
}

/// Sends `message` with `fds`, bypassing the sender's own checks.
fn send_raw(socket: BorrowedFd<'_>, message: &[u8], fds: &[BorrowedFd<'_>]) -> Fallible {
    let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(2))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    if !fds.is_empty() && !control.push(SendAncillaryMessage::ScmRights(fds)) {
        return Err("the control buffer does not hold the fds".into());
    }
    sendmsg(
        socket,
        &[IoSlice::new(message)],
        &mut control,
        SendFlags::NOSIGNAL,
    )?;
    Ok(())
}

/// Positive, negative and boundary in one: 1,000 refusals of six kinds, each
/// one refused, and the descriptor count back at its baseline.
#[test]
fn a_thousand_refused_messages_leave_the_descriptor_count_at_its_baseline() -> Fallible {
    let (sender, receiver) = endpoints()?;
    let object = memfd("scaena-fd-leak", NV12_SIZE)?;
    let mut out = LineBuffer::new();
    let valid = encode_request(&request(11)?, &mut out)?.to_vec();
    let text = String::from_utf8(valid.clone())?;
    let pad = MAX_LINE_BYTES.saturating_sub(valid.len()).saturating_add(1);
    let kinds: [(Vec<u8>, usize); 6] = [
        (valid.clone(), 2),
        (valid.clone(), 0),
        (text.replace("\"2.0\"", "\"1.0\"").into_bytes(), 1),
        (text.replacen('{', "{\"fd\":9,", 1).into_bytes(), 1),
        (
            text.replacen('{', &format!("{{{}", " ".repeat(pad)), 1)
                .into_bytes(),
            1,
        ),
        (valid, 1),
    ];
    let mut buffer = LineBuffer::new();
    let baseline = open_descriptors()?;
    let mut refused = 0_usize;
    for (message, fds) in kinds.iter().cycle().take(REFUSED) {
        let copies: Vec<BorrowedFd<'_>> = (0..*fds).map(|_| object.as_fd()).collect();
        send_raw(sender.as_fd(), message, &copies)?;
        let outcome = receiver
            .receive_request(&mut buffer)
            .map_err(|error| error.to_string())
            .and_then(|(request, fd)| {
                attach::check(request.frame, fd).map_err(|error| error.to_string())
            });
        if outcome.is_err() {
            refused = refused.saturating_add(1);
        }
    }
    assert_eq!(refused, REFUSED, "every message must be refused");
    assert_eq!(open_descriptors()?, baseline);
    Ok(())
}

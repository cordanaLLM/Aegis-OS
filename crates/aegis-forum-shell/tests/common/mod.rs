// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the P05 integration tests.
//!
//! The payloads are built with the producers' own types and encoded with the
//! producers' own encoders, so a test here exercises the contract as
//! `aegis-justitia` and `aegis-tellus` define it, not a copy. Nothing here
//! uses `unwrap` or `expect`: a fixture that cannot be built is returned as
//! an error.

#![allow(dead_code)]

use std::io::{self, Read, Write};
use std::time::Duration;

use aegis_forum_shell::jsonrpc::DeadlineStream;
use aegis_justitia::{
    ActionType, AgentId, DecisionRequest, DecisionRequestVersion, Identity, MakerId,
    OversightClass, RequestId, RequiredApproval, RiskTier, TargetResource, UnixSeconds,
};
use aegis_tellus::{CarbonTelemetry, CorrelationId, GridIntensity, Provenance, SciRate, Watts};

/// What every test returns.
pub type Fallible<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// A decision request with request id `request`, tier A, standard oversight,
/// a 300-second window: the M14 fixture's values.
pub fn decision_request(request: &str) -> Fallible<DecisionRequest> {
    let tier = RiskTier::TierAConsequential;
    let class = OversightClass::Standard;
    Ok(DecisionRequest {
        schema: DecisionRequestVersion::V1,
        edge: DecisionRequest::EDGE,
        correlation_id: Identity::parse("intent-0001")?,
        request_id: RequestId::parse(request)?,
        agent_id: AgentId::parse("agent-01")?,
        maker: MakerId::parse("maker-alice")?,
        proposed_action: ActionType::FileDeletion,
        target: TargetResource::parse("/var/lib/aegis/example")?,
        risk_tier: tier,
        oversight: class,
        required_approval: RequiredApproval::for_action(tier, class),
        created_at: UnixSeconds::new(1_000),
        due_at: UnixSeconds::new(1_300),
    })
}

/// The producer's own encoding of `request`.
pub fn decision_payload(request: &DecisionRequest) -> Fallible<String> {
    let mut buffer = aegis_justitia::PayloadBuffer::new();
    Ok(request.encode_into(&mut buffer)?.to_owned())
}

/// A simulated telemetry update at the default grid intensity.
pub fn telemetry(watts: f64, rate: f64) -> Fallible<CarbonTelemetry> {
    Ok(CarbonTelemetry::new(
        CorrelationId::parse("m16-telemetry-0001")?,
        Watts::new(watts)?,
        SciRate::new(rate)?,
        GridIntensity::DEFAULT,
        Provenance::Simulated,
    ))
}

/// The producer's own encoding of `update`.
pub fn telemetry_payload(update: &CarbonTelemetry) -> Fallible<String> {
    let mut buffer = aegis_tellus::PayloadBuffer::new();
    Ok(update.encode_into(&mut buffer)?.to_owned())
}

/// One JSON-RPC 2.0 message line, newline included; `id` makes it a call,
/// `None` a Notification.
pub fn message(method: &str, params: &str, id: Option<u32>) -> String {
    let id = id.map(|id| format!(",\"id\":{id}")).unwrap_or_default();
    format!("{{\"jsonrpc\":\"2.0\",\"method\":\"{method}\",\"params\":{params}{id}}}\n")
}

/// A stream over one end of a `socketpair(2)`, arming `SO_RCVTIMEO` and
/// `SO_SNDTIMEO` through the standard library.
#[cfg(unix)]
pub struct Mocked(pub std::os::unix::net::UnixStream);

#[cfg(unix)]
impl Read for Mocked {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

#[cfg(unix)]
impl Write for Mocked {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[cfg(unix)]
impl DeadlineStream for Mocked {
    fn arm_read_deadline(&mut self, deadline: Duration) -> io::Result<()> {
        self.0.set_read_timeout(Some(deadline))
    }

    fn arm_write_deadline(&mut self, deadline: Duration) -> io::Result<()> {
        self.0.set_write_timeout(Some(deadline))
    }
}

/// The timeout the fixture sets on both ends of the mocked stream before a
/// test starts: longer than any deadline a test arms or asserts, so it only
/// bounds a read or write the code under test left without a deadline.
#[cfg(unix)]
pub const BACKSTOP: Duration = Duration::from_secs(10);

/// A mocked `AF_UNIX` stream: the shell's end and the producer's end.
///
/// Both ends carry [`BACKSTOP`] as their timeout, so no test can block
/// without bound. The code under test replaces the shell end's timeout with
/// its own deadline before every read and write; if it failed to, a read
/// would end only at the backstop, which is past the bound
/// `a_missed_read_deadline_ends_the_session` asserts, so that test fails
/// instead of hanging.
#[cfg(unix)]
pub fn socketpair() -> Fallible<(Mocked, std::os::unix::net::UnixStream)> {
    let (shell, producer) = std::os::unix::net::UnixStream::pair()?;
    for end in [&shell, &producer] {
        end.set_read_timeout(Some(BACKSTOP))?;
        end.set_write_timeout(Some(BACKSTOP))?;
    }
    Ok((Mocked(shell), producer))
}

/// An in-memory stream that replays `input` in reads of at most `chunk`
/// bytes, then reports its deadline as missed, and counts every read and
/// every armed deadline.
pub struct Scripted {
    pub input: Vec<u8>,
    pub offset: usize,
    pub chunk: usize,
    pub reads: usize,
    pub armed_reads: usize,
    pub armed_writes: usize,
    pub written: Vec<u8>,
    pub end_with_eof: bool,
}

impl Scripted {
    /// A stream replaying `input`; `end_with_eof` ends it cleanly, otherwise
    /// the read after the last byte misses its deadline.
    pub fn new(input: &[u8], chunk: usize, end_with_eof: bool) -> Self {
        Self {
            input: input.to_vec(),
            offset: 0,
            chunk,
            reads: 0,
            armed_reads: 0,
            armed_writes: 0,
            written: Vec::new(),
            end_with_eof,
        }
    }
}

impl Read for Scripted {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reads = self.reads.saturating_add(1);
        let rest = self.input.get(self.offset..).unwrap_or_default();
        if rest.is_empty() {
            return if self.end_with_eof {
                Ok(0)
            } else {
                Err(io::Error::from(io::ErrorKind::WouldBlock))
            };
        }
        let take = rest.len().min(self.chunk).min(buf.len());
        let (Some(source), Some(target)) = (rest.get(..take), buf.get_mut(..take)) else {
            return Ok(0);
        };
        target.copy_from_slice(source);
        self.offset = self.offset.saturating_add(take);
        Ok(take)
    }
}

impl Write for Scripted {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.written.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl DeadlineStream for Scripted {
    fn arm_read_deadline(&mut self, _deadline: Duration) -> io::Result<()> {
        self.armed_reads = self.armed_reads.saturating_add(1);
        Ok(())
    }

    fn arm_write_deadline(&mut self, _deadline: Duration) -> io::Result<()> {
        self.armed_writes = self.armed_writes.saturating_add(1);
        Ok(())
    }
}

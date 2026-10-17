// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The hardened `justitia-interceptor` unit, reviewed as a contract.
//!
//! The unit text lives at `crates/aegis-justitia/contracts/`
//! `justitia-interceptor.service`. It is **a contract file, not an installed
//! unit**: nothing in this repository copies it into a systemd unit directory,
//! enables it or starts it, and the `ExecStart` binary it names is not built by
//! this milestone (the crate ships a library and declares no `[[bin]]`).
//!
//! What is executable about it is the review. [`review`] is ordinary library
//! code that reads unit text and refuses it when a hardening directive is
//! missing, so "the unit declares `IPAddressDeny=any`" is a check that fails
//! when someone deletes the line, rather than a sentence in a document.
//! `IPAddressDeny=any` is the directive the source ties to the Zero-SaaS
//! requirement (export-015 `fcbe2caed363`).
//!
//! The unit text must also carry [`CONTRACT_MARKER`] in its header, so a copy
//! that has had the "this is not an installed unit" warning stripped out of it
//! fails review as well.

/// Scalar upper bound, in bytes, on a unit file this module will review.
pub const MAX_UNIT_BYTES: usize = 8192;

/// Scalar upper bound on the lines this module will read from a unit file.
pub const MAX_UNIT_LINES: usize = 256;

/// The marker the unit header must carry to say what the file is.
pub const CONTRACT_MARKER: &str = "CONTRACT-ONLY";

/// The directives the hardened unit must declare, each on a line of its own.
pub const REQUIRED_DIRECTIVES: [&str; 5] = [
    "IPAddressDeny=any",
    "ProtectSystem=strict",
    "ProtectHome=yes",
    "Type=notify",
    "Restart=always",
];

/// The start rate-limit settings the unit must write down, each once, in its
/// `[Unit]` section and with a positive value.
///
/// `StartLimitIntervalSec=0` beside `Restart=always` switches the limit off,
/// and a gate failing on every start would then be restarted without bound
/// (D108); inheriting the manager's default leaves the policy unwritten
/// (issue #232). Both are refused.
pub const START_LIMIT_DIRECTIVES: [&str; 2] = [START_LIMIT_INTERVAL, START_LIMIT_BURST];

/// The interval of the start rate limit, in seconds.
const START_LIMIT_INTERVAL: &str = "StartLimitIntervalSec";

/// The number of starts the interval admits.
const START_LIMIT_BURST: &str = "StartLimitBurst";

/// The older spelling systemd still reads as `StartLimitIntervalSec=`; a line
/// with it counts as a second declaration of the interval.
const LEGACY_INTERVAL_KEY: &str = "StartLimitInterval";

/// The systemd directories an installed unit would live in.
///
/// The contract file is refused a home in any of them; [`is_installed_path`]
/// is how a check states that rather than assuming it.
pub const INSTALL_DIRECTORIES: [&str; 4] = [
    "/etc/systemd/system/",
    "/usr/lib/systemd/system/",
    "/lib/systemd/system/",
    "/run/systemd/system/",
];

/// Reasons a unit text fails the contract review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum UnitContractError {
    /// A required hardening directive is absent.
    #[error("the unit does not declare {directive}")]
    MissingDirective {
        /// The directive that was not found.
        directive: &'static str,
    },
    /// The header does not say that the file is a contract, not a unit.
    #[error("the unit header does not carry the {CONTRACT_MARKER} marker")]
    MissingContractMarker,
    /// A start rate-limit setting is absent.
    #[error("the unit does not set {directive}")]
    MissingStartLimit {
        /// The setting that was not found.
        directive: &'static str,
    },
    /// A start rate-limit setting is declared more than once, so a later line
    /// could override the value the review read.
    #[error("the unit sets {directive} more than once")]
    RepeatedStartLimit {
        /// The setting declared twice.
        directive: &'static str,
    },
    /// A start rate-limit setting is not a positive whole number (seconds,
    /// optionally suffixed `s`, for the interval).
    #[error("the unit sets {directive} to a value that is not a positive whole number")]
    InvalidStartLimit {
        /// The setting whose value was refused.
        directive: &'static str,
    },
    /// A start rate-limit setting sits outside the `[Unit]` section.
    #[error("the unit sets {directive} outside its [Unit] section")]
    MisplacedStartLimit {
        /// The setting found in another section.
        directive: &'static str,
    },
    /// The text is longer than this module will read.
    #[error("the unit text of {actual} bytes exceeds the bound of {max}")]
    TooLong {
        /// The scalar bound in bytes.
        max: usize,
        /// The width observed.
        actual: usize,
    },
}

/// Records which required directives one line declares.
fn mark(present: &mut [bool; REQUIRED_DIRECTIVES.len()], line: &str) {
    for (slot, directive) in present.iter_mut().zip(REQUIRED_DIRECTIVES.iter()) {
        if line == *directive {
            *slot = true;
        }
    }
}

/// Returns the first required directive that no line declared.
fn first_missing(present: [bool; REQUIRED_DIRECTIVES.len()]) -> Option<&'static str> {
    for (slot, directive) in present.iter().zip(REQUIRED_DIRECTIVES.iter()) {
        if !*slot {
            return Some(directive);
        }
    }
    None
}

/// Returns which start rate-limit setting a directive key names, if any.
fn start_limit_directive(key: &str) -> Option<&'static str> {
    match key {
        START_LIMIT_INTERVAL | LEGACY_INTERVAL_KEY => Some(START_LIMIT_INTERVAL),
        START_LIMIT_BURST => Some(START_LIMIT_BURST),
        _ => None,
    }
}

/// Returns `true` when `value` is an accepted value for `directive`.
///
/// The accepted forms are an allow-list: ASCII digits that are not all zero,
/// and for the interval the same followed by `s`. `0`, `infinity` and other
/// time-span spellings are refused.
fn start_limit_value_ok(directive: &str, value: &str) -> bool {
    let digits = match value.strip_suffix('s') {
        Some(stripped) if directive == START_LIMIT_INTERVAL => stripped,
        _ => value,
    };
    !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && digits.bytes().any(|byte| byte != b'0')
}

/// The state of one bounded pass over unit text.
#[derive(Default)]
struct Scan {
    marked: bool,
    in_unit_section: bool,
    present: [bool; REQUIRED_DIRECTIVES.len()],
    start_limits: [bool; START_LIMIT_DIRECTIVES.len()],
}

impl Scan {
    /// Records one non-comment line.
    fn directive(&mut self, line: &str) -> Result<(), UnitContractError> {
        if line.starts_with('[') {
            self.in_unit_section = line == "[Unit]";
            return Ok(());
        }
        mark(&mut self.present, line);
        let Some((key, value)) = line.split_once('=') else {
            return Ok(());
        };
        let Some(directive) = start_limit_directive(key.trim()) else {
            return Ok(());
        };
        for (seen, known) in self.start_limits.iter_mut().zip(START_LIMIT_DIRECTIVES) {
            if known == directive && *seen {
                return Err(UnitContractError::RepeatedStartLimit { directive });
            }
            *seen = *seen || known == directive;
        }
        if !self.in_unit_section {
            return Err(UnitContractError::MisplacedStartLimit { directive });
        }
        if !start_limit_value_ok(directive, value.trim()) {
            return Err(UnitContractError::InvalidStartLimit { directive });
        }
        Ok(())
    }

    /// Returns the first setting the pass did not find.
    fn finish(self) -> Result<(), UnitContractError> {
        if !self.marked {
            return Err(UnitContractError::MissingContractMarker);
        }
        if let Some(directive) = first_missing(self.present) {
            return Err(UnitContractError::MissingDirective { directive });
        }
        for (seen, directive) in self.start_limits.iter().zip(START_LIMIT_DIRECTIVES.iter()) {
            if !*seen {
                return Err(UnitContractError::MissingStartLimit { directive });
            }
        }
        Ok(())
    }
}

/// Reviews unit text against the hardened-unit contract.
///
/// Comment lines are read for [`CONTRACT_MARKER`] and ignored for directives,
/// so a directive that survives only inside a comment does not count as
/// declared.
///
/// # Errors
///
/// Returns [`UnitContractError::TooLong`] past [`MAX_UNIT_BYTES`],
/// [`UnitContractError::MissingContractMarker`] when the header marker is
/// absent, [`UnitContractError::MissingDirective`] naming the first
/// required directive the text does not declare, and a start-limit error when
/// a setting of [`START_LIMIT_DIRECTIVES`] is absent, repeated (the older
/// `StartLimitInterval=` spelling included), outside `[Unit]` or not a
/// positive whole number.
pub fn review(text: &str) -> Result<(), UnitContractError> {
    if text.len() > MAX_UNIT_BYTES {
        return Err(UnitContractError::TooLong {
            max: MAX_UNIT_BYTES,
            actual: text.len(),
        });
    }
    let mut scan = Scan::default();
    for line in text.lines().take(MAX_UNIT_LINES) {
        let trimmed = line.trim();
        scan.marked = scan.marked || trimmed.contains(CONTRACT_MARKER);
        if !trimmed.starts_with('#') && !trimmed.starts_with(';') {
            scan.directive(trimmed)?;
        }
    }
    scan.finish()
}

/// Returns `true` when `path` is inside a directory systemd loads units from.
///
/// The contract file must never be at such a path: it is reviewed, not
/// installed, and this is the check that says so.
#[must_use]
pub fn is_installed_path(path: &str) -> bool {
    INSTALL_DIRECTORIES
        .iter()
        .any(|directory| path.starts_with(directory))
}

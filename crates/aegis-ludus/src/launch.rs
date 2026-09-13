// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded launch-command-line validator (REQ-P11-02, REQ-P11-07).
//!
//! The scaffold splits the launch command line on whitespace, refuses a line of
//! more than [`MAX_LAUNCH_ARGS`] arguments, scans for one token prefix, and
//! then writes
//!
//! ```text
//! is_authenticated: token_found || !args.is_empty()
//! ```
//!
//! into the session. That last expression is the defect this module exists to
//! remove: it reports a line that carries **no** token as authenticated for the
//! sole reason that it carried some argument, and it reports the empty line as
//! unauthenticated for the same reason rather than for a considered one. A
//! boolean cannot tell the three cases apart, so the answer here is
//! [`AuthenticationOutcome`], which has one variant for each of them.
//!
//! [`AuthenticationOutcome::is_authenticated`] is `true` only for
//! [`AuthenticationOutcome::TokenPresent`]. The scaffold's rule is kept beside
//! it as [`AuthenticationOutcome::scaffold_verdict`] rather than deleted, so
//! the disagreement is a value a test can assert instead of a sentence in a
//! commit message: the two answers differ on exactly one of the three
//! outcomes.
//!
//! # What is validated, and what is not
//!
//! A token is checked for its **prefix, its length and its character set**. It
//! is not parsed, not decoded and not authenticated against anything: this
//! crate reaches no platform, opens no socket and verifies no credential, so a
//! line carrying a well-formed token establishes that the line is well formed
//! and nothing more. [`AUTH_TOKEN_PREFIX`] is the recorded prefix, kept from
//! the scaffold.

use crate::error::LudusError;

/// Scalar upper bound on the arguments one launch command line may carry.
///
/// Sixty-four is the scaffold's own `MAX_LAUNCH_ARGS`, recorded as REQ-P11-07.
pub const MAX_LAUNCH_ARGS: usize = 64;

/// Scalar upper bound, in bytes, on one launch argument.
///
/// The scaffold declares no per-argument bound at all, so this one is chosen
/// here: it is what keeps a command line an inline, `Copy` value instead of a
/// heap-allocated one, and the scaffold's own longest sample argument is 48
/// bytes.
pub const MAX_ARGUMENT_BYTES: usize = 64;

/// The token prefix the scaffold scans a launch command line for.
pub const AUTH_TOKEN_PREFIX: &str = "+connect_auth_token=";

/// One validated launch argument of at most [`MAX_ARGUMENT_BYTES`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LaunchArgument {
    bytes: [u8; MAX_ARGUMENT_BYTES],
    len: u8,
}

impl LaunchArgument {
    /// The padding an unused command-line slot holds. Never a valid argument.
    const PADDING: Self = Self {
        bytes: [0u8; MAX_ARGUMENT_BYTES],
        len: 0,
    };

    /// Parses `raw` into one launch argument.
    ///
    /// # Errors
    ///
    /// Returns [`LudusError::LaunchArgumentOutOfRange`] when `raw` is empty or
    /// longer than [`MAX_ARGUMENT_BYTES`], and
    /// [`LudusError::LaunchArgumentCharset`] when it carries a byte outside
    /// printable ASCII. Whitespace is outside that set, which is why an
    /// argument can never be silently split or joined. The input is never
    /// truncated or sanitised.
    pub fn parse(raw: &str) -> Result<Self, LudusError> {
        let source = raw.as_bytes();
        let len = u8::try_from(source.len())
            .ok()
            .filter(|len| *len > 0 && source.len() <= MAX_ARGUMENT_BYTES)
            .ok_or(LudusError::LaunchArgumentOutOfRange {
                max: MAX_ARGUMENT_BYTES,
                actual: source.len(),
            })?;
        let mut bytes = [0u8; MAX_ARGUMENT_BYTES];
        for (slot, byte) in bytes.iter_mut().zip(source.iter()) {
            if !byte.is_ascii_graphic() {
                return Err(LudusError::LaunchArgumentCharset);
            }
            *slot = *byte;
        }
        Ok(Self { bytes, len })
    }

    /// Returns the validated bytes, without the trailing padding.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..usize::from(self.len)).unwrap_or(&[])
    }

    /// Returns the argument length in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns `false`; a parsed argument is never empty by construction.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns `true` when the argument starts with [`AUTH_TOKEN_PREFIX`].
    ///
    /// A prefix match, and deliberately nothing more. What follows the prefix
    /// is not parsed, decoded or checked against any platform.
    #[must_use]
    pub fn carries_launch_token(&self) -> bool {
        self.as_bytes().starts_with(AUTH_TOKEN_PREFIX.as_bytes())
    }
}

/// What a launch command line says about authentication.
///
/// Three variants, because the scaffold's boolean conflates three cases. Only
/// [`Self::TokenPresent`] is an authenticated line here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum AuthenticationOutcome {
    /// Some argument carried [`AUTH_TOKEN_PREFIX`].
    TokenPresent,
    /// Arguments were present and none of them carried the token prefix.
    NoLaunchToken,
    /// The command line carried no argument at all.
    EmptyCommandLine,
}

impl AuthenticationOutcome {
    /// Every outcome, in declaration order.
    pub const ALL: [Self; 3] = [
        Self::TokenPresent,
        Self::NoLaunchToken,
        Self::EmptyCommandLine,
    ];

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::TokenPresent => "token-present",
            Self::NoLaunchToken => "no-launch-token",
            Self::EmptyCommandLine => "empty-command-line",
        }
    }

    /// Returns `true` only when a launch token was actually present.
    #[must_use]
    pub const fn is_authenticated(self) -> bool {
        matches!(self, Self::TokenPresent)
    }

    /// Returns what the scaffold's `token_found || !args.is_empty()` concludes.
    ///
    /// Kept so the disagreement is a value rather than a claim: it differs from
    /// [`Self::is_authenticated`] on [`Self::NoLaunchToken`] and agrees on the
    /// other two. `tests/launch_validator.rs` pins all three.
    #[must_use]
    pub const fn scaffold_verdict(self) -> bool {
        !matches!(self, Self::EmptyCommandLine)
    }
}

/// A validated launch command line of at most [`MAX_LAUNCH_ARGS`] arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchCommandLine {
    arguments: [LaunchArgument; MAX_LAUNCH_ARGS],
    len: usize,
}

impl Default for LaunchCommandLine {
    fn default() -> Self {
        Self::empty()
    }
}

impl LaunchCommandLine {
    /// Builds the command line that carries no argument.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            arguments: [LaunchArgument::PADDING; MAX_LAUNCH_ARGS],
            len: 0,
        }
    }

    /// Splits `raw` on ASCII whitespace and validates every argument.
    ///
    /// # Errors
    ///
    /// Returns [`LudusError::TooManyLaunchArguments`] when the line carries
    /// more than [`MAX_LAUNCH_ARGS`] arguments, and propagates
    /// [`LaunchArgument::parse`] for the first argument it refuses. A line that
    /// is refused produces no command line at all, so an over-long line cannot
    /// be truncated into an admissible one.
    pub fn parse(raw: &str) -> Result<Self, LudusError> {
        let ceiling = MAX_LAUNCH_ARGS.saturating_add(1);
        let counted = raw.split_ascii_whitespace().take(ceiling).count();
        if counted > MAX_LAUNCH_ARGS {
            return Err(LudusError::TooManyLaunchArguments {
                max: MAX_LAUNCH_ARGS,
                at_least: counted,
            });
        }
        let mut line = Self::empty();
        for (slot, token) in line
            .arguments
            .iter_mut()
            .zip(raw.split_ascii_whitespace().take(MAX_LAUNCH_ARGS))
        {
            *slot = LaunchArgument::parse(token)?;
            line.len = line.len.saturating_add(1);
        }
        Ok(line)
    }

    /// Returns the validated arguments, without the trailing padding.
    #[must_use]
    pub fn arguments(&self) -> &[LaunchArgument] {
        self.arguments.get(..self.len).unwrap_or(&[])
    }

    /// Returns how many arguments the line carries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the line carries no argument at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns what the line says about authentication.
    ///
    /// The empty line is answered explicitly with
    /// [`AuthenticationOutcome::EmptyCommandLine`] rather than falling out of
    /// a scan that found nothing.
    #[must_use]
    pub fn authenticate(&self) -> AuthenticationOutcome {
        if self.is_empty() {
            return AuthenticationOutcome::EmptyCommandLine;
        }
        for argument in self.arguments().iter().take(MAX_LAUNCH_ARGS) {
            if argument.carries_launch_token() {
                return AuthenticationOutcome::TokenPresent;
            }
        }
        AuthenticationOutcome::NoLaunchToken
    }
}

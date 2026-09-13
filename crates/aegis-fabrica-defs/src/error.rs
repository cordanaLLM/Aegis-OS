// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The refusals: everything this crate reads as "not a valid definition".
//!
//! A variant exists for one of two reasons, and the documentation on each says
//! which:
//!
//! * systemd itself refuses the input, and the crate refuses it for the same
//!   reason so a definition can be checked without running systemd;
//! * systemd accepts the input after printing a diagnostic, and the crate
//!   refuses it because accepting it would let a definition drift away from
//!   what the tool will actually do (REQ-CI-01).

/// Why a definition was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DefinitionError {
    /// The file has more lines than the scalar bound allows.
    #[error("{name}: {lines} lines exceeds the bound of {bound}")]
    TooManyLines {
        /// The file the parser was reading.
        name: String,
        /// How many lines it holds.
        lines: usize,
        /// The bound it passed.
        bound: usize,
    },
    /// A single line is longer than the scalar bound allows.
    #[error("{name}:{line}: line is longer than the bound of {bound} bytes")]
    LineTooLong {
        /// The file the parser was reading.
        name: String,
        /// The 1-based line number.
        line: usize,
        /// The bound it passed.
        bound: usize,
    },
    /// The file declares more sections or assignments than the bound allows.
    #[error("{name}: more than {bound} {what}")]
    TooMany {
        /// The file the parser was reading.
        name: String,
        /// What overflowed, as a plural noun.
        what: &'static str,
        /// The bound it passed.
        bound: usize,
    },
    /// A line is neither a comment, a section header nor a `Key=Value` pair.
    ///
    /// systemd refuses the same line with `Missing '=', ignoring line`.
    #[error("{name}:{line}: not a comment, a section header or a Key=Value pair")]
    MalformedLine {
        /// The file the parser was reading.
        name: String,
        /// The 1-based line number.
        line: usize,
    },
    /// An assignment appeared before any section header.
    #[error("{name}:{line}: assignment {key}= appears before any section header")]
    KeyOutsideSection {
        /// The file the parser was reading.
        name: String,
        /// The 1-based line number.
        line: usize,
        /// The key that has no section.
        key: String,
    },
    /// A section header repeats.
    ///
    /// systemd merges repeated sections. The crate refuses them: a reader of a
    /// reviewed definition should not have to hold two `[Partition]` blocks in
    /// their head to know what one file declares.
    #[error("{name}:{line}: section [{section}] is declared more than once")]
    DuplicateSection {
        /// The file the parser was reading.
        name: String,
        /// The 1-based line number of the repeat.
        line: usize,
        /// The section that repeats.
        section: String,
    },
    /// A key repeats inside one section.
    ///
    /// systemd takes the last assignment. The crate refuses the repeat for the
    /// same reason it refuses a duplicate section.
    #[error("{name}:{line}: key {key}= is assigned more than once in [{section}]")]
    DuplicateKey {
        /// The file the parser was reading.
        name: String,
        /// The 1-based line number of the repeat.
        line: usize,
        /// The section holding the repeat.
        section: String,
        /// The key that repeats.
        key: String,
    },
    /// A section this definition kind does not define.
    #[error("{name}:{line}: unknown section [{section}]")]
    UnknownSection {
        /// The file the parser was reading.
        name: String,
        /// The 1-based line number.
        line: usize,
        /// The section that is not defined.
        section: String,
    },
    /// A key systemd does not define.
    ///
    /// systemd prints `Unknown key '...' in section [...], ignoring` and
    /// carries on, which is how the imported `Subsystem=`, `BtrfsSubvolumes=`
    /// and transfer keys survived review. The crate refuses them.
    #[error("{name}:{line}: unknown key {key}= in section [{section}]")]
    UnknownKey {
        /// The file the parser was reading.
        name: String,
        /// The 1-based line number.
        line: usize,
        /// The section holding the key.
        section: String,
        /// The key systemd does not define.
        key: String,
    },
    /// A mandatory key is absent.
    ///
    /// systemd refuses a partition with no `Type=` and a transfer whose source
    /// has no `MatchPattern=`.
    #[error("{name}: [{section}] has no {key}=, which is mandatory")]
    MissingKey {
        /// The file the parser was reading.
        name: String,
        /// The section that is incomplete.
        section: String,
        /// The key that is missing.
        key: &'static str,
    },
    /// A size, count or boolean value could not be read.
    #[error("{name}:{line}: {key}= value {value:?} is not a valid {expected}")]
    MalformedValue {
        /// The file the parser was reading.
        name: String,
        /// The 1-based line number.
        line: usize,
        /// The key whose value is unreadable.
        key: String,
        /// The value as written.
        value: String,
        /// What a readable value would have been.
        expected: &'static str,
    },
    /// `SizeMinBytes=` is larger than `SizeMaxBytes=`.
    ///
    /// systemd refuses it with `SizeMinBytes= larger than SizeMaxBytes=,
    /// refusing.` and exits 1.
    #[error("{name}: SizeMinBytes={minimum} is larger than SizeMaxBytes={maximum}")]
    InvertedSize {
        /// The file the parser was reading.
        name: String,
        /// The lower bound, in bytes.
        minimum: u64,
        /// The upper bound, in bytes.
        maximum: u64,
    },
}

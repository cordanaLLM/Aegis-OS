// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: shared contract scaffolding across subsystem crates.
//!
//! One file, included by path into the contracts module of every crate
//! that defines or consumes cross-subsystem versioned payloads
//! (`#[path = "../../../contract_scaffold.rs"]`), extending the D78
//! precedent to contract scaffolds. It is not a crate and has no manifest:
//! a workspace member would enter every dependency closure and carry a
//! permanent name.
//!
//! What D116 folds:
//! - [`Correlation`]: the pair of schema and correlation identifier.
//! - [`PayloadBuffer`]: the fixed-capacity stack buffer enforcing byte bounds.
//! - Peekers and decode/encode helpers: [`peek_correlation`], [`peek_schema`],
//!   [`classify`], [`decode_text`], [`encode_into`].
//! - [`define_contract_error!`]: macro defining the common refusal variants
//!   and their correlation projection.
//! - Contract validation helpers: [`ensure_edge`], [`decode_validated`].

#![allow(dead_code)]

/// What a refusal is about: the contract, and the payload when it could be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Correlation<S, I> {
    /// The contract the payload claimed to satisfy.
    schema: S,
    /// The correlation identifier, when the decoder could read one.
    id: Option<I>,
}

impl<S: Copy, I: Copy> Correlation<S, I> {
    /// Builds a correlation from a schema and the identifier, if one was read.
    #[must_use]
    pub const fn new(schema: S, id: Option<I>) -> Self {
        Self { schema, id }
    }

    /// Returns the contract the payload claimed to satisfy.
    #[must_use]
    pub const fn schema(&self) -> S {
        self.schema
    }

    /// Returns the correlation identifier, when the decoder could read one.
    #[must_use]
    pub const fn id(&self) -> Option<I> {
        self.id
    }
}

impl<S: core::fmt::Display, I: core::fmt::Display> core::fmt::Display for Correlation<S, I> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.id {
            Some(id) => write!(f, "{} [{}]", self.schema, id),
            None => write!(f, "{} [uncorrelated]", self.schema),
        }
    }
}

/// A fixed buffer holding one encoded contract payload.
///
/// Encoding writes into a caller-supplied buffer rather than returning an
/// owned string, so a payload that encodes successfully costs no heap and the
/// byte bound is enforced by the buffer itself rather than by a length check
/// after the fact.
#[derive(Debug, Clone, Copy)]
pub struct PayloadBuffer<const N: usize> {
    /// The backing byte array holding the serialized payload.
    bytes: [u8; N],
    /// The number of valid bytes written.
    len: usize,
}

impl<const N: usize> Default for PayloadBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> PayloadBuffer<N> {
    /// Builds an empty buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bytes: [0u8; N],
            len: 0,
        }
    }

    /// Returns the bytes written so far.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or(&[])
    }

    /// Returns the encoded payload as text, when it is valid UTF-8.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        core::str::from_utf8(self.as_bytes()).ok()
    }

    /// Returns the number of bytes written.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when nothing has been encoded into the buffer.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Resets the buffer and serializes `value` into it.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::TooLong`] when the serialized bytes exceed `N`,
    /// and [`WriteError::Malformed`] when the serialized bytes are not valid UTF-8.
    pub(crate) fn write_json<T: serde::Serialize>(
        &mut self,
        value: &T,
    ) -> Result<&str, WriteError> {
        self.len = 0;
        let mut tail: &mut [u8] = &mut self.bytes;
        let written = serde_json::to_writer(&mut tail, value);
        let remaining = tail.len();
        if written.is_err() {
            return Err(WriteError::TooLong(N));
        }
        self.len = N.saturating_sub(remaining);
        match self.as_str() {
            Some(text) => Ok(text),
            None => Err(WriteError::Malformed),
        }
    }
}

/// Errors occurring during payload buffer encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WriteError {
    /// Payload exceeded the maximum byte bound.
    TooLong(usize),
    /// Payload was not valid UTF-8 text.
    Malformed,
}

/// A contract schema version identifier.
pub(crate) trait ContractSchema: Copy + core::fmt::Display {
    /// Returns the stable version tag the payload's `schema` field carries.
    fn schema_tag(self) -> &'static str;
}

/// A contract payload correlation identifier.
pub(crate) trait ContractIdentifier: for<'de> serde::Deserialize<'de> + Copy {
    /// Returns the identifier as bytes.
    fn id_bytes(&self) -> &[u8];
}

/// Maps refusal variants for contract decoding and encoding.
pub(crate) trait ContractRefusal<S, I>: Sized {
    /// Constructs an `UnknownVersion` refusal.
    fn unknown_version(correlation: Correlation<S, I>) -> Self;
    /// Constructs a `Malformed` refusal.
    fn malformed(correlation: Correlation<S, I>) -> Self;
    /// Constructs a `PayloadTooLong` refusal.
    fn payload_too_long(correlation: Correlation<S, I>, max: usize) -> Self;
}

/// The lenient view of a payload's `schema` field, and of nothing else.
#[derive(serde::Deserialize)]
#[serde(bound(deserialize = "I: serde::Deserialize<'de>"))]
struct PeekSchema<I> {
    #[serde(default)]
    schema: Option<I>,
}

/// The lenient view of a payload's correlation identifier, and of nothing else.
#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[serde(bound(deserialize = "I: serde::Deserialize<'de>"))]
struct PeekCorrelation<I> {
    #[serde(default)]
    correlation_id: Option<I>,
}

/// Reads the correlation identifier a payload carried, if it carried one.
pub(crate) fn peek_correlation<I>(text: &str) -> Option<I>
where
    I: for<'de> serde::Deserialize<'de>,
{
    serde_json::from_str::<PeekCorrelation<I>>(text)
        .ok()
        .and_then(|row| row.correlation_id)
}

/// Reads the contract version a payload claimed, if it claimed a readable one.
pub(crate) fn peek_schema<I>(text: &str) -> Option<I>
where
    I: for<'de> serde::Deserialize<'de>,
{
    serde_json::from_str::<PeekSchema<I>>(text)
        .ok()
        .and_then(|row| row.schema)
}

/// Classifies a decode failure into a correlated refusal.
pub(crate) fn classify<S, I, E>(schema: S, text: &str) -> E
where
    S: ContractSchema,
    I: ContractIdentifier,
    E: ContractRefusal<S, I>,
{
    let correlation = Correlation::new(schema, peek_correlation::<I>(text));
    match peek_schema::<I>(text) {
        Some(tag) if tag.id_bytes() != schema.schema_tag().as_bytes() => {
            E::unknown_version(correlation)
        }
        _ => E::malformed(correlation),
    }
}

/// Decodes one payload of `schema`, refusing anything over the byte bound.
///
/// # Errors
///
/// Returns [`ContractRefusal::payload_too_long`] when the text exceeds `N` bytes,
/// or maps the error via [`classify`].
pub(crate) fn decode_text<T, S, I, const N: usize, E>(schema: S, text: &str) -> Result<T, E>
where
    T: for<'de> serde::Deserialize<'de>,
    S: ContractSchema,
    I: ContractIdentifier,
    E: ContractRefusal<S, I>,
{
    if text.len() > N {
        return Err(E::payload_too_long(Correlation::new(schema, None), N));
    }
    match serde_json::from_str::<T>(text) {
        Ok(value) => Ok(value),
        Err(_) => Err(classify::<S, I, E>(schema, text)),
    }
}

/// Encodes `value` into `buffer` and returns the rendered payload.
///
/// # Errors
///
/// Returns [`ContractRefusal::payload_too_long`] on buffer overflow,
/// or [`ContractRefusal::malformed`] if invalid UTF-8.
pub(crate) fn encode_into<'b, T, S, I, const N: usize, E>(
    value: &T,
    correlation: Correlation<S, I>,
    buffer: &'b mut PayloadBuffer<N>,
) -> Result<&'b str, E>
where
    T: serde::Serialize,
    S: Copy,
    I: Copy,
    E: ContractRefusal<S, I>,
{
    match buffer.write_json(value) {
        Ok(text) => Ok(text),
        Err(WriteError::TooLong(max)) => Err(E::payload_too_long(correlation, max)),
        Err(WriteError::Malformed) => Err(E::malformed(correlation)),
    }
}

/// Runs validation on a successfully decoded payload.
///
/// # Errors
///
/// Propagates decode errors and invariant validation errors.
pub(crate) fn decode_validated<T, E>(
    decoded: Result<T, E>,
    validate: impl FnOnce(&T) -> Result<(), E>,
) -> Result<T, E> {
    let value = decoded?;
    validate(&value)?;
    Ok(value)
}

/// Trait for edge enumerations that expose a stable edge name.
pub(crate) trait EdgeName {
    /// Returns the edge name.
    fn edge_name(&self) -> &'static str;
}

/// Validates that an edge matches the expected edge, returning an error on mismatch.
///
/// # Errors
///
/// Returns `make_err` if `found != expected`.
pub(crate) fn ensure_edge<E: Copy + PartialEq + EdgeName, C, Err>(
    found: E,
    expected: E,
    correlation: C,
    make_err: impl FnOnce(C, &'static str) -> Err,
) -> Result<(), Err> {
    if found != expected {
        return Err(make_err(correlation, found.edge_name()));
    }
    Ok(())
}

/// Defines a refusal enum with standard contract refusal variants and correlation accessor.
macro_rules! define_contract_error {
    (
        $schema:ty, $id:ty;
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $(
                $(#[$var_meta:meta])*
                $var:ident { $($fields:tt)* } $(,)?
            )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
        #[non_exhaustive]
        $vis enum $name {
            /// The payload names a contract version this build does not admit.
            #[error("{correlation}: the payload names a contract version this build does not admit")]
            UnknownVersion {
                /// What the refusal is about.
                correlation: Correlation,
            },
            /// The payload is not a well-formed instance of its contract.
            #[error("{correlation}: the payload is not a well-formed instance of this contract")]
            Malformed {
                /// What the refusal is about.
                correlation: Correlation,
            },
            /// The payload does not fit the contract's scalar byte bound.
            #[error("{correlation}: the payload does not fit the {max}-byte contract bound")]
            PayloadTooLong {
                /// What the refusal is about.
                correlation: Correlation,
                /// The scalar bound in bytes.
                max: usize,
            },
            $(
                $(#[$var_meta])*
                $var { $($fields)* },
            )*
        }

        impl $name {
            /// Returns what the refusal is about.
            #[must_use]
            pub const fn correlation(&self) -> Correlation {
                match self {
                    Self::UnknownVersion { correlation }
                    | Self::Malformed { correlation }
                    | Self::PayloadTooLong { correlation, .. }
                    $( | Self::$var { correlation, .. } )* => *correlation,
                }
            }
        }

        impl contract_scaffold::ContractRefusal<$schema, $id> for $name {
            fn unknown_version(correlation: Correlation) -> Self {
                Self::UnknownVersion { correlation }
            }
            fn malformed(correlation: Correlation) -> Self {
                Self::Malformed { correlation }
            }
            fn payload_too_long(correlation: Correlation, max: usize) -> Self {
                Self::PayloadTooLong { correlation, max }
            }
        }
    };
}

/// Scaffolds contract boilerplate: display, schema/identifier traits, and codec helpers.
macro_rules! scaffold_contract {
    ($schema:ident, $id:ident, $max_bytes:ident, $error:ident) => {
        scaffold_contract!($schema, $id, $max_bytes, $error, custom_encode);

        #[allow(dead_code)]
        pub(crate) fn encode_into<'b, T: serde::Serialize>(
            value: &T,
            correlation: Correlation,
            buffer: &'b mut PayloadBuffer,
        ) -> Result<&'b str, $error> {
            contract_scaffold::encode_into(value, correlation, &mut *buffer)
        }
    };
    ($schema:ident, $id:ident, $max_bytes:ident, $error:ident, custom_encode) => {
        impl core::fmt::Display for $schema {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(self.tag())
            }
        }

        impl contract_scaffold::ContractSchema for $schema {
            fn schema_tag(self) -> &'static str {
                self.tag()
            }
        }

        impl contract_scaffold::ContractIdentifier for $id {
            fn id_bytes(&self) -> &[u8] {
                self.as_bytes()
            }
        }

        #[allow(dead_code)]
        pub(crate) fn peek_correlation(text: &str) -> Option<$id> {
            contract_scaffold::peek_correlation(text)
        }

        #[allow(dead_code)]
        pub(crate) fn decode_text<T: for<'de> serde::Deserialize<'de>>(
            schema: $schema,
            text: &str,
        ) -> Result<T, $error> {
            contract_scaffold::decode_text::<T, $schema, $id, $max_bytes, $error>(schema, text)
        }
    };
    ($schema:ident, $id:ident, $edge:ty, $max_bytes:ident, $error:ident) => {
        scaffold_contract!($schema, $id, $max_bytes, $error);

        impl contract_scaffold::EdgeName for $edge {
            fn edge_name(&self) -> &'static str {
                self.name()
            }
        }
    };
}

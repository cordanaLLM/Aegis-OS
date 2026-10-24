// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: shared contract codec scaffolding across subsystem crates.
//!
//! One file, included by path into the contracts module of every crate
//! that uses [`scaffold_contract!`] (`#[path = "../../../contract_codec_scaffold.rs"]`),
//! after `contract_scaffold.rs`. It holds `scaffold_contract!`, which generates
//! contract schema and identifier trait implementations along with decoding and
//! encoding helpers.

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

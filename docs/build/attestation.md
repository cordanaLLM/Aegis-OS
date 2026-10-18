<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# TPM2 PCR quote attestation over swtpm

Status: recorded observations from milestone M20 epic E20-1, reference profile,
2026-10-18

Milestone M20 epic E20-1 implements the TPM 2.0 attestation slice on swtpm
(issue #65). An audit record from P06 Justitia is bound to the platform state
through a TPM2 quote over PCRs 0, 4, 7, and 11, using the canonical SHA-256
audit-record digest as qualifying data (`extraData`).

The unit tests for the pure judges, argument builders and selection parsers run
inside `make verify-all`. Executing the live TPM quote and verification requires
swtpm and tpm2-tools, so it runs here:

```sh
make verify-attestation
```

**A pass is development evidence on swtpm only (D79).** The gate executes
against a private `swtpm socket` instance in an ephemeral temporary directory.
The host TPM is never touched. The pass is not rooted in a firmware-verified boot
chain (M11), and it closes no image, boot, hardware, or release gate.

## What is tracked

| Path | Role |
| :--- | :--- |
| `crates/aegis-justitia/examples/record_digest.rs` | prints canonical digest of fixed RecordBody; `other` for rejected |
| `tools/verify_attestation.py` | gate: starts private swtpm, drives tpm2-tools, judges 5 cases |
| `tools/test_verify_attestation.py` | unit tests for pure judges, parsers, and builders in verify-all |
| `docs/build/attestation.md` | this document: architecture, measured facts, limits, cases |
| `Makefile` | target `verify-attestation` |

## What the gate does

1. Checks host capabilities: Linux, `swtpm`, `tpm2-tools` (`tpm2_quote`,
   `tpm2_checkquote`, `tpm2_print`, `tpm2_createek`, `tpm2_createak`,
   `tpm2_pcrextend`, `tpm2_flushcontext`), `cargo`, and
   `/usr/lib/libtss2-tcti-swtpm.so`. If any capability is missing, it prints
   `SKIP: <reason>; the attestation gate did not run.` and exits 0.
2. Creates an ephemeral directory with `0700` permissions.
3. Obtains canonical record digests from `crates/aegis-justitia` using
   `cargo run --locked --offline -q -p aegis-justitia --example record_digest`
   (and with `other` argument).
4. Launches `swtpm socket --tpm2` in the foreground as a session child with
   `--flags not-need-init,startup-clear`.
5. Explicitly sets `TPM2TOOLS_TCTI=swtpm:path=<sock>` for all child tools.
   Every child command is executed with a 20 s timeout.
6. Creates an RSA Endorsement Key (EK) and an ECC Attestation Key (AK) with
   ECDSA over SHA-256.
7. Extends PCRs 0, 4, 7, and 11 with fixed benchmark values.
8. Generates a TPM2 quote over PCRs 0, 4, 7, and 11 with the record digest as
   qualifying data, and verifies the five cases below.
9. Tears down the swtpm process group via `host.end_session` and removes the
   ephemeral directory.

## The cases

| Case | Criterion | Passes when |
| :--- | :--- | :--- |
| `attest/quote-verifies` | positive | `tpm2_checkquote` exits 0 and selection judge accepts |
| `attest/other-record-fails` | negative | checkquote with different record digest exits non-zero |
| `attest/pcr-state-changed-fails` | negative | quote after PCR 7 extend fails against old PCR file |
| `attest/exact-selection-accepted` | boundary | selection judge accepts exact PCR 0,4,7,11 quote |
| `attest/missing-pcr11-rejected` | boundary | quote over 0,4,7 exits 0 in checkquote, judge rejects |

## Measured facts and TPM2 semantics

Measured on the reference profile on 2026-10-18 (`swtpm 0.10.2`, `tpm2-tools
5.8`, `/usr/lib/libtss2-tcti-swtpm.so`):

- **Context management without resource manager**:
  `swtpm socket` does not include an in-kernel or daemon resource manager.
  Without explicit `tpm2_flushcontext -t` (transient) and `-s` (saved session)
  calls after `createek`, `createak`, and `quote`, the TPM quickly exhausts
  object slots and returns error `0x902` (`TPM_RC_MEMORY`: out of memory for
  object contexts).
- **Selection enforcement is client responsibility**:
  `tpm2_checkquote` verifies the cryptographic signature over `TPMS_ATTEST` and
  verifies that the provided PCR file hashes to the `pcrDigest` in the
  attestation structure. However, `tpm2_checkquote` does **not** enforce which
  PCRs were selected: a quote over only PCR 0, 4, 7 verifies with exit code 0.
  The gate's own judge (`judge_selection`) inspects `tpm2_print -t TPMS_ATTEST`
  to verify that PCR 11 is present.
- **PCR selection bitmap encoding**:
  In TPM 2.0 `TPMS_PCR_SELECTION`, the `pcrSelect` array is little-endian per
  byte:
  - Byte 0 holds PCRs 0..7: bits 0, 4, and 7 give `0x91` (`0b10010001`).
  - Byte 1 holds PCRs 8..15: bit 3 (PCR 11) gives `0x08` (`0b00001000`).
  - Byte 2 holds PCRs 16..23: zero gives `0x00`.
  The resulting hex string printed by `tpm2_print` is `910800`. A quote missing
  PCR 11 prints `910000` and is rejected by the gate's judge, naming PCR 11.

## The recorded run

Reference profile, 2026-10-18, `swtpm 0.10.2`, `tpm2-tools 5.8`:

```text
CASE attest/quote-verifies pass
CASE attest/other-record-fails pass
CASE attest/pcr-state-changed-fails pass
CASE attest/exact-selection-accepted pass
CASE attest/missing-pcr11-rejected pass
NOTE versions: swtpm 0.10.2, tpm2-tools 5.8;
  development evidence; not rooted in a firmware-verified boot chain (M11)
RESULT pass
```

The live cases were run once against two planted defects on the same day:

- `tpm2_checkquote` called without `-q`, so the qualifying data goes unchecked:
  `attest/quote-verifies` and `attest/missing-pcr11-rejected` fail and the run
  ends `RESULT fail`.
- A selection judge that accepts every bitmap: `attest/missing-pcr11-rejected`
  fails.

`tools/test_verify_attestation.py` holds the bitmap `910008` as a negative
case: it selects PCR 19 in place of PCR 11 and must be refused.

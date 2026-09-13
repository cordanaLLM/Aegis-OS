// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! What the reference profile actually has for the two P11 credentials.
//!
//! P11 names two pieces of hardware in `planning/components.json`: a TPM2
//! security chip for PCR-sealed receipts (REQ-P11-04, REQ-P11-06) and a
//! FIDO2-capable authenticator. Those are different situations on this profile,
//! and a stub that covered both the same way would hide the difference.
//!
//! [`REFERENCE_PROFILE_PROBES`] records three commands run by hand at milestone
//! M08 and the strings that came back. Two found a TPM2; the third found no
//! authenticator at all. So the FIDO2 half of P11 is a **procurement
//! dependency** -- there is nothing to write code against and no stub here
//! pretends otherwise -- while the TPM2 half is a deferred binding: the chip is
//! present and no key is bound to it by this repository.
//!
//! # What this register is not
//!
//! It is **not** a capability check. This crate executes no command, no test
//! re-runs any of these, and a host with no TPM2 and no authenticator passes
//! every test in this crate. Read it as a dated note about one machine on one
//! day, and re-run the commands if the answer matters.
//!
//! `planning/hardware-profile.json` is the register of record for the profile's
//! capabilities and already carries the TPM2 row; these three strings are what
//! P11's own credentials looked like when the crate was written.

/// One credential probe, run by hand and recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CredentialProbe {
    /// The credential the probe was about.
    pub capability: &'static str,
    /// The command that was run.
    pub command: &'static str,
    /// What the command printed, verbatim, or what its absence looked like.
    pub observed: &'static str,
    /// Whether the probe found the credential.
    pub present: bool,
    /// The day the probe was run.
    pub probed_on: &'static str,
}

impl CredentialProbe {
    /// Returns `true` when the credential has to be bought before it can be
    /// used, because the probe found nothing to bind to.
    #[must_use]
    pub const fn is_procurement_dependency(&self) -> bool {
        !self.present
    }
}

/// The three probes run on the reference profile at milestone M08.
///
/// Recorded, not re-run. Two found a TPM 2.0; the third found no FIDO2
/// authenticator among the connected USB devices.
pub const REFERENCE_PROFILE_PROBES: [CredentialProbe; 3] = [
    CredentialProbe {
        capability: "tpm2-device",
        command: "ls /sys/class/tpm/",
        observed: "tpm0",
        present: true,
        probed_on: "2026-09-13",
    },
    CredentialProbe {
        capability: "tpm2-major-version",
        command: "cat /sys/class/tpm/tpm0/tpm_version_major",
        observed: "2",
        present: true,
        probed_on: "2026-09-13",
    },
    CredentialProbe {
        capability: "fido2-authenticator",
        command: "lsusb | grep -iE 'yubi|fido|solo|token|nitro'",
        observed: "no output; the filter matched none of the 17 connected devices and exited 1",
        present: false,
        probed_on: "2026-09-13",
    },
];

/// The capability name of the probe that found the platform's TPM device node.
pub const TPM2_DEVICE: &str = "tpm2-device";

/// The capability name of the probe that read the TPM's major version.
pub const TPM2_MAJOR_VERSION: &str = "tpm2-major-version";

/// The capability name of the probe that looked for a FIDO2 authenticator.
pub const FIDO2_AUTHENTICATOR: &str = "fido2-authenticator";

/// Returns the recorded probe for `capability`, if one was run.
///
/// The sweep is bounded by the length of [`REFERENCE_PROFILE_PROBES`], which
/// is a fixed array.
#[must_use]
pub fn recorded_probe(capability: &str) -> Option<CredentialProbe> {
    REFERENCE_PROFILE_PROBES
        .into_iter()
        .take(REFERENCE_PROFILE_PROBES.len())
        .find(|row| row.capability == capability)
}

// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P03 engineering mandate, recorded rather than enforced.
//!
//! Three recorded requirements describe P03 in numbers this crate cannot
//! measure: a whole-subsystem power budget (REQ-P03-01), six named protection
//! boundaries (REQ-P03-02), and an isolation model with a latency budget
//! (REQ-P03-06). This module is a register for them, in the shape
//! `aegis-janus-lifecycle` uses for decision D13: static data naming what was
//! recorded and which source recorded it.
//!
//! **It admits and refuses nothing.** No watt is measured here, no latency is
//! timed, and no boundary is enforced by any code in this crate. Measuring the
//! power budget needs the RAPL and device counters M25 reaches over real
//! hardware; measuring the latency budget needs a real `VFIO` path. Recording a
//! number is not evidence that a system meets it, and `tests/mandate.rs` tests
//! the register against the requirement identifiers, not against a device.

/// A private source, cited by export identifier and digest prefix only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Citation {
    /// The export identifier of the private source.
    pub export: &'static str,
    /// The first twelve hexadecimal characters of that export's sha256.
    pub sha256_prefix: &'static str,
}

/// One of the six protection-domain boundaries REQ-P03-02 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProtectionBoundary {
    /// What the protection domain covers.
    Scope,
    /// What the domain may spend.
    Budget,
    /// What the domain is allowed to observe.
    SensedState,
    /// What the domain is allowed to cause.
    CausalOperation,
    /// Which devices the domain may reach.
    Topology,
    /// Whether an operation of the domain can be undone.
    Reversibility,
}

impl ProtectionBoundary {
    /// All six boundaries, in the order REQ-P03-02 names them.
    pub const ALL: [Self; 6] = [
        Self::Scope,
        Self::Budget,
        Self::SensedState,
        Self::CausalOperation,
        Self::Topology,
        Self::Reversibility,
    ];

    /// Returns the stable name this boundary is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Scope => "scope",
            Self::Budget => "budget",
            Self::SensedState => "sensed-state",
            Self::CausalOperation => "causal-operation",
            Self::Topology => "topology",
            Self::Reversibility => "reversibility",
        }
    }
}

/// The recorded P03 engineering mandate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineeringMandate {
    /// The whole-subsystem power budget, in watts (REQ-P03-01).
    pub power_budget_watts: u32,
    /// The direct-DMA latency budget, in microseconds (REQ-P03-06).
    pub latency_budget_micros: u32,
    /// The isolation model the subsystem graph records (REQ-P03-06).
    pub isolation_model: &'static str,
    /// The six protection-domain boundaries (REQ-P03-02).
    pub boundaries: [ProtectionBoundary; 6],
    /// The recorded requirements this register stands for.
    pub records: [&'static str; 3],
    /// What no code in this crate does about any of it.
    pub enforcement: &'static str,
    /// The private sources that state the mandate.
    pub sources: [Citation; 2],
}

/// The P03 mandate as recorded at milestone M01.
pub const P03_MANDATE: EngineeringMandate = EngineeringMandate {
    power_budget_watts: 20,
    latency_budget_micros: 10,
    isolation_model: "VFIO group isolation with one IOMMU domain per group",
    boundaries: ProtectionBoundary::ALL,
    records: ["REQ-P03-01", "REQ-P03-02", "REQ-P03-06"],
    enforcement: "recorded only: this crate measures no power, times no latency and \
                  enforces no boundary; M25 is where a real VFIO path is reached",
    sources: [
        Citation {
            export: "export-012",
            sha256_prefix: "9b502c76509b",
        },
        Citation {
            export: "export-062",
            sha256_prefix: "1ce919ed54bb",
        },
    ],
};

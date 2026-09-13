<!-- markdownlint-disable MD013 -->
# ADR-0002: Exclude the Steamworks SDK from the Aegis image

- **Status**: Accepted
- **Date**: 2026-09-13
- **Authors**: @lusoris (decision), roadmap decision D12

## Context

The P11 Ludus blueprint wraps the proprietary C++ Steamworks SDK through bindgen
(requirement REQ-P11-01). The concept's comparative matrix labels P11's
governing constraint as free and open-source software compliance, and the
repository licenses its technical material under EUPL-1.2. Vendoring or linking
a closed SDK conflicts with that constraint and would need a separate licence
review for redistribution in a signed image.

## Decision

The Aegis image contains no Steamworks SDK and no Steamworks integration. P11
aegis-ludus covers only free and open-source integration paths, such as
gamescope capture through P08 and Discord rich presence over its local IPC
socket. The Steamworks parts of the P11 blueprint are superseded.

## Consequences

- **Positive**: no proprietary SDK in the build graph, the image, or the licence
  inventory; P11 stays consistent with its FOSS-compliance constraint and the
  EUPL scope.
- **Negative**: Steam-specific features in the blueprint (Steamworks API calls,
  in-game transaction attestation that depends on them) are out of scope. Users
  who want Steam run the upstream Steam client as a separately installed
  application outside the image.
- M08's exit criteria include a negative test proving that the image and the P11
  crate build without any Steamworks SDK.
- REQ-P11-01 stays in the requirement set as source evidence and is marked
  superseded by this ADR.

## References

- Roadmap decision D12, `docs/roadmap/README.md` (Risks and decisions).
- Requirement REQ-P11-01, `docs/roadmap/requirements.md`.
- Maintainer answer, 2026-09-13, to "P11 Ludus depends on the proprietary
  Steamworks SDK, while the concept labels P11's constraint as FOSS compliance.
  How to reconcile?": "Exclude from image".

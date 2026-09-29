<!-- markdownlint-disable MD013 -->
# Architectural Decision Records (ADRs)

This directory documents key architectural decisions using immutable,
append-only four-digit numbering (fleet invariant HISS-14: public contracts and
records are append-only).

| Number | Date | Title | Status |
| :--- | :--- | :--- | :--- |
| [0000](0000-template.md) | — | ADR template | Template |
| [0001](0001-pure-rust-compositor.md) | 2026-09-13 | Implement the P04 compositor in pure Rust | Accepted |
| [0002](0002-exclude-steamworks-from-image.md) | 2026-09-13 | Exclude the Steamworks SDK from the Aegis image | Accepted |
| [0003](0003-display-runtime-p17-scaena.md) | 2026-09-28 | Add P17 aegis-scaena, where the compositor composes | Accepted |
| [0004](0004-native-p05-shell-on-gpui.md) | 2026-09-29 | Build the whole P05 shell natively in Rust on gpui | Accepted |

An accepted ADR is not edited. ADR-0004 supersedes decision 3 of ADR-0003 in
part, and ADR-0003 stays as it was accepted.

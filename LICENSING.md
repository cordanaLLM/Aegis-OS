<!-- markdownlint-disable MD013 -->
# Licensing

Copyright © 2026 Lusoris and Aegis OS contributors

This repository uses a split licence. The licence follows the nature of the
material, not merely its filename extension. A file-specific copyright or
licence notice overrides this general allocation for that file or identified
part. `REUSE.toml` records the same allocation in machine-readable form.

## Technical material: EUPL v1.2 or later

Project-authored software and technical execution material is licensed under
the European Union Public Licence v1.2 or, at your option, a later version as
permitted by Article 5 of that licence. The enclosed licence text has the SPDX
identifier `EUPL-1.2`; the later-version choice is stated in prose because
SPDX does not define a separate `-or-later` identifier for the EUPL.

This scope includes project-authored:

- source code, build and image configuration, and deployment code;
- eBPF programs, kernel configuration fragments, and partition definitions;
- scripts, command-line tools, daemons, and application code;
- automated tests and test harnesses;
- configuration files, data schemas, manifests, and workflow definitions;
- agent operating instructions and governance configuration; and
- package metadata, lockfiles, and other technical build infrastructure.

The verbatim EUPL v1.2 English text is in [`LICENSE`](LICENSE) and
[`LICENSES/EUPL-1.2.txt`](LICENSES/EUPL-1.2.txt).

Text provenance: the enclosed text is the canonical English text published by
the [SPDX License List](https://spdx.org/licenses/EUPL-1.2), which links the
[European Commission source](https://interoperable-europe.ec.europa.eu/collection/eupl/eupl-text-eupl-12).
`make verify-all` fails if the enclosed text differs from the pinned digest.

Kernel-loaded eBPF programs must additionally declare the licence string the
kernel requires to load GPL-only helpers. The EUPL v1.2 Appendix lists GPL v2
and v3 as compatible licences; each activated eBPF source records its kernel
licence string next to its SPDX header.

## Original research and presentation: CC BY-SA 4.0

Original project prose, architectural specifications, decision records,
roadmaps, diagrams, and the presentation of rendered documentation are
licensed under the Creative Commons Attribution-ShareAlike 4.0 International
Public License (`CC-BY-SA-4.0`). The software that renders or verifies that
material remains covered by the EUPL scope above.

When redistributing or adapting material in this scope, attribute it as:

> Aegis OS — Copyright © 2026 Lusoris and Aegis OS contributors — CC BY-SA 4.0

Include a link to this repository when reasonably practicable, identify
changes, and comply with the ShareAlike terms. The verbatim legal code is in
[`LICENSES/CC-BY-SA-4.0.txt`](LICENSES/CC-BY-SA-4.0.txt).

Text provenance: the enclosed legal code is copied verbatim from the
[Creative Commons official plain-text legal code](https://creativecommons.org/licenses/by-sa/4.0/legalcode.txt).
`make verify-all` fails if the enclosed text differs from the pinned digest.

## Material not relicensed by this repository

The licences above apply only to rights that the project contributors are
entitled to license. In particular:

- Private planning inputs under `.workingdir/` are not part of this repository
  and are not published or licensed by it.
- Linux kernel, systemd, mkosi, PipeWire, and every other third-party
  component, specification, standard, law, image, and dependency retain their
  respective terms and notices.
- A citation, hyperlink, bibliography entry, factual reference, or lawful
  quotation does not relicense the cited or quoted work.

No patent, trademark, publicity, privacy, personality, or other right is
granted beyond the rights expressly covered by the applicable licence.

## Contributions

By contributing, you represent that you have the necessary rights and agree
that your contribution will be licensed under the applicable scope above. Mark
third-party material and exceptions explicitly; do not assume that academic or
public availability makes a work open-licensed.

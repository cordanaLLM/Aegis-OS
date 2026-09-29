# ui

Reserved for UI component manifests and lockfiles: P12 concordia design tokens
and P15 hestia app. Activation requires package manifests, lockfiles, and an
axe-core accessibility test per component. P05 forum-shell is no longer a UI
package here: under D101 (ADR-0004, 2026-09-29) the whole shell is native Rust
on gpui in `crates/aegis-forum-shell` (M16, M28), and its accessibility check is
an AccessKit tree check, not an axe-core scan. The JavaScript and Svelte that
stay here are linted for HISS-01, HISS-04 and HISS-08 by ESLint in the pinned
container (M16, D100): the configuration, its local no-self-recursion rule and
the planted violations the gate lints are in `concordia-tokens/`, beside the
code they cover, until cordanaLLM/praetor#589 ships a scanner.

`concordia-tokens/` holds the P12 token file, one Svelte 5 component and the
accessibility suite that `make verify-all` runs through `tools/verify_a11y.py`
(milestone M04, `docs/build/accessibility-harness.md`). P12 is still a proposal
in `planning/components.json`. The other candidates are indexed under
`.workingdir/prepared/scaffold/ui/` (private, gitignored; not present in a
clone); they are inactive proposal data.
See `planning/components.json` and `docs/integration/stack.md` for activation
prerequisites and shared ownership. No native pass is claimed by this directory.

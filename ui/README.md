# ui

Reserved for UI component manifests and lockfiles: P05 forum-shell, P12
concordia design tokens, and P15 hestia app. Activation requires package
manifests, lockfiles, and an axe-core accessibility test per component.

`concordia-tokens/` holds the P12 token file, one Svelte 5 component and the
accessibility suite that `make verify-all` runs through `tools/verify_a11y.py`
(milestone M04, `docs/build/accessibility-harness.md`). P12 is still a proposal
in `planning/components.json`. The other candidates are indexed under
`.workingdir/prepared/scaffold/ui/` (private, gitignored; not present in a
clone); they are inactive proposal data.
See `planning/components.json` and `docs/integration/stack.md` for activation
prerequisites and shared ownership. No native pass is claimed by this directory.

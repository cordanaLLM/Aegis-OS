# build

Reserved for OS-specific configuration adapters (P01 mkosi/UKI synthesis, P02
repart, sysupdate and TPM2 enrolment) that feed Imago image construction and
consume Nucleus kernel artifacts. Activation requires pinned producer schemas
and one locally tested request/result pair.

Current candidates are indexed under `.workingdir/prepared/scaffold/build/`
(private, gitignored; not present in a clone); they are inactive proposal data.
See `planning/components.json` and `docs/integration/stack.md` for activation
prerequisites and shared ownership. No native pass is claimed by this directory.

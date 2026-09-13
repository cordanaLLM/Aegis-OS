# bpf

Reserved for eBPF sources with explicit kernel ABI and verifier evidence: P06
`action_gate`, P07 `scx_cake`, and P13 `kepler_power`. Each program needs a
pinned kernel version, a clang/libbpf toolchain admitted through the template
matrix, and verifier logs from a real kernel.

Current candidates are indexed under `.workingdir/prepared/scaffold/bpf/` (private,
gitignored; not present in a clone); they are inactive proposal data. See
`planning/components.json` and `docs/integration/stack.md` for activation
prerequisites and shared ownership. No native pass is claimed by this
directory.

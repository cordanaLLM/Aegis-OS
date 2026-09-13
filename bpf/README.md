# bpf

The eBPF fixtures milestone M19 puts through the host kernel's verifier, and the
loader that keeps the verifier's log for every load. Full evidence, including the
quoted verifier output and the scheduler attach/detach readings, is in
[`docs/build/bpf.md`](../docs/build/bpf.md).

| File | Subsystem | What it is |
| :--- | :--- | :--- |
| `aegis_bpf_abi.h` | shared | record layout and declared bounds, included by both the objects and the loader |
| `action_gate.bpf.c` | P06 | BPF LSM program on `lsm/bprm_check_security`, observe-only |
| `kepler_power.bpf.c` | P13 | tracepoint program on `sched/sched_switch` |
| `scx_cake.bpf.c` | P07 | sched_ext `struct_ops`. Loaded through the verifier, never attached |
| `scx_cake_stub.bpf.c` | P07 | the boundary fixture: a `struct_ops` whose nine declared handlers are empty |
| `loader/aegis_bpf_probe.c` | gate | retains the per-program verifier log, holds the deadlines, attaches and detaches |

Run it with `make verify-bpf`. That target is deliberately **not** part of `make
verify-all`, and the `Makefile` records why: it needs CAP_BPF, CAP_PERFMON, a
kernel with BTF and BPF LSM, and for one case the machine's CPU scheduler, none
of which CI has. The half that needs no kernel runs in `verify-all` as
`tools/test_bpf_objects.py`.

Three things are worth knowing before reading the sources:

- **Nothing here denies, schedules or measures anything.** `action_gate` returns
  only `AEGIS_ACTION_ALLOW`, which is zero; `scx_cake` is never attached;
  `scx_cake_stub` is attached with `SCX_OPS_SWITCH_PARTIAL`, so no task is ever
  scheduled through it. Every threshold in `aegis_bpf_abi.h` is a declared
  budget, and the two values that could be mistaken for measurements carry
  `_UNMEASURED` in their names.
- **The negative variants are generated, not tracked.** Each is the positive
  source with exactly one `// AEGIS-MUTATE-BEGIN <name>` … `// AEGIS-MUTATE-END
  <name>` region deleted, so the rejected object differs from the accepted one in
  that bound and nothing else. Do not remove a marker pair without moving its
  case.
- **A pass is a non-qualifying local fixture.** The verifier that accepted these
  objects belongs to a kernel this repository neither builds nor configures, so
  it does not close M10's Nucleus-kernel verification and closes no image, boot,
  hardware or release gate.

`.workingdir/prepared/scaffold/bpf/` (private, gitignored, not present in a
clone) holds the imported proposal sketches these were rewritten from. They are
inactive proposal data; `bpf/scx_cake.bpf.c` records its provenance against
`extra/scx-scheds 1.1.3-2` in its own header (decision D66).

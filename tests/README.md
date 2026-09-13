# tests

Reserved for real component, image, boot and hardware acceptance tests. Every
public interface needs positive, negative and boundary tests (HISS-15);
image/boot/hardware evidence is retained here only when produced by the real
gates.

## Active fixtures

`systemd/` holds the recorded negative and boundary inputs for the P01/P02
definition gate (M03): a partition with no `Type=`, an inverted size pair, an
equal size pair, a key systemd ignores while exiting 0, a transfer built from
keys `sysupdate.d(5)` does not define, and a single-slot transfer. Each file
states the exit code and diagnostic systemd 261 answers it with.
`tools/verify_systemd_definitions.py` runs them; nothing here is image, boot or
hardware evidence.

Current candidates are indexed under `.workingdir/prepared/proposals/` (private,
gitignored; not present in a clone); they are inactive proposal data. See
`planning/components.json` and `docs/integration/stack.md` for activation
prerequisites and shared ownership. No native pass is claimed by this directory.

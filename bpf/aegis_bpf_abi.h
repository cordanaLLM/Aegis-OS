/* SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
 * SPDX-License-Identifier: EUPL-1.2
 *
 * The ABI the M19 eBPF fixtures share with their loader.
 *
 * Both sides include this file and neither declares the layout twice: the BPF
 * objects get the fixed-width types from `vmlinux.h`, the loader gets them from
 * <linux/types.h>, and the struct below is the same object file offsets in both
 * translation units. Nothing here is a kernel ABI; it is the fixture's own.
 *
 * Every literal in this header is a declared bound, not a measurement. The two
 * that would be mistaken for measurements carry _UNMEASURED in their name.
 */

#ifndef AEGIS_BPF_ABI_H
#define AEGIS_BPF_ABI_H

/* The ring buffer the P06 action gate reserves into. Its name is read back from
 * the loaded object by the gate, so renaming it here fails the gate rather than
 * silently producing a consumer that polls nothing.
 */
#define AEGIS_ACTION_RINGBUF_BYTES (256 * 1024)

/* Verdicts the P06 fixture can record. The fixture is observe-only: it never
 * returns a denial of its own, so AEGIS_ACTION_ALLOW is the only value the LSM
 * hook ever returns and it is zero, which is "permit" for bprm_check_security.
 * A denial reached from this object would be an unreviewed policy change on a
 * hook that runs on every exec.
 */
#define AEGIS_ACTION_ALLOW 0
/* Bound on the recorded path. `bprm->filename` is the path execve was asked for,
 * and it is the field a gate must key on: at `bprm_check_security` the task's
 * `comm` is still the name of the process that called execve, not the program
 * being executed, so a consumer matching on comm matches the wrong thing. That
 * was observed here before this field existed. A longer path is truncated, and
 * truncation is visible because the consumer compares a suffix.
 */
#define AEGIS_ACTION_PATH_BYTES 64
#define AEGIS_ACTION_KIND_EXEC 1u
#define AEGIS_VERDICT_OBSERVED 0u

/* Per-cgroup slots the P13 power probe accumulates into. A power of two, because
 * the fixture bounds its index with a mask rather than a comparison; the gate's
 * negative case removes exactly that mask.
 */
#define AEGIS_KEPLER_SLOTS 8u
#define AEGIS_KEPLER_SLOT_MASK (AEGIS_KEPLER_SLOTS - 1u)
#define AEGIS_KEPLER_MAX_CGROUPS 1024

/* DECLARED, NEVER MEASURED. The imported P13 proposal multiplies scheduler
 * runtime by a flat 15 W to produce a number in microjoules and calls the result
 * energy. It is not energy: no RAPL MSR, no powercap sysfs file and no ACPI
 * meter is read anywhere in this object, so the product is runtime in disguise.
 * The literal is kept, at the proposal's value, so that M21 can replace it with
 * a counter read and the diff shows which figure changed; the name carries
 * _UNMEASURED so no consumer can treat the field as a measurement by accident.
 */
#define AEGIS_KEPLER_TDP_MILLIWATT_UNMEASURED 15000ULL

/* Scheduling tiers the P07 fixture classifies into, and the burst thresholds
 * that separate them. Declared budgets from the P07 register rows, not observed
 * latencies: nothing in this repository has measured a burst yet.
 */
#define AEGIS_CAKE_TIERS 4u
#define AEGIS_CAKE_TIER_CRITICAL 0u
#define AEGIS_CAKE_TIER_INTERACTIVE 1u
#define AEGIS_CAKE_TIER_FRAME 2u
#define AEGIS_CAKE_TIER_BULK 3u
#define AEGIS_CAKE_BURST_CRITICAL_NS_DECLARED 100000ULL
#define AEGIS_CAKE_BURST_INTERACTIVE_NS_DECLARED 2000000ULL
#define AEGIS_CAKE_BURST_FRAME_NS_DECLARED 8000000ULL
#define AEGIS_CAKE_EWMA_WEIGHT_OLD 3ULL
#define AEGIS_CAKE_EWMA_WEIGHT_NEW 1ULL
#define AEGIS_CAKE_EWMA_DIVISOR 4ULL
#define AEGIS_CAKE_TASKS 65536
/* Dispatch-queue identifiers. Any value with bit 63 clear is a user DSQ; the
 * builtin identifiers all set it, so this base cannot collide with SCX_DSQ_LOCAL,
 * SCX_DSQ_GLOBAL or SCX_DSQ_BYPASS.
 */
#define AEGIS_CAKE_DSQ_BASE 0x41454749u

/* The watchdog bound the stub scheduler declares for itself. sched_ext ejects a
 * scheduler that leaves a runnable task un-run for longer than this, which is
 * why attaching the stub is recoverable without any action from the operator.
 */
#define AEGIS_CAKE_STUB_TIMEOUT_MS 5000u

struct aegis_action_event {
	__u64 pid_tgid;
	__u64 cgroup_id;
	__u32 kind;
	__u32 verdict;
	__u32 argc;
	__u32 reserved;
	char comm[16];
	char filename[AEGIS_ACTION_PATH_BYTES];
};

struct aegis_power_sample {
	__u64 cgroup_id;
	__u64 last_timestamp_ns;
	__u64 runtime_ns[AEGIS_KEPLER_SLOTS];
	/* Runtime multiplied by a declared constant. Not a meter reading. */
	__u64 pseudo_energy_uj_unmeasured;
};

struct aegis_cake_task_ctx {
	__u64 run_start_ns;
	__u64 ewma_burst_ns;
	__u32 tier;
	__u32 reserved;
};

#endif /* AEGIS_BPF_ABI_H */

// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// P07 `scx_cake`: the four-tier burst classifier, as a sched_ext struct_ops.
//
// PROVENANCE. This is an Aegis original that shares a name with a distribution
// binary and derives from neither. `/usr/bin/scx_cake` on the reference profile
// belongs to `extra/scx-scheds 1.1.3-2` (upstream https://github.com/sched-ext/scx,
// GPL-2.0-only); no line of that project's scheduler is copied here, no upstream
// commit is vendored, and this object is not a fork of it. What it does descend
// from is the imported P07 proposal sketch under `.workingdir/prepared/scaffold/`
// (private, gitignored, immutable proposal data), rewritten here. Decision D66.
//
// WHAT MILESTONE M19 DOES WITH THIS OBJECT: it loads it through the verifier and
// stops. It is never attached. Attaching a scheduler takes over CPU scheduling
// for the whole machine, and the only object this milestone attaches is the
// all-stub `scx_cake_stub.bpf.c`, which cannot receive a task at all. The tier
// thresholds below are declared budgets from the P07 register rows; no burst
// has been measured anywhere in this repository, and loading an object measures
// nothing.
//
// The proposal sketch this replaces had an `enqueue` that classified a task and
// then dispatched it nowhere -- the dispatch call was commented out. Attached,
// that scheduler stalls every runnable task until the sched_ext watchdog ejects
// it. The handlers below dispatch for real, which is why this object is a
// rewrite rather than an import.

#include "vmlinux.h"

#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

#include "aegis_bpf_abi.h"

char LICENSE[] SEC("license") = "GPL";

// sched_ext kfuncs. Each prototype is the one the running kernel's own BTF
// declares, read back with `bpftool btf dump file /sys/kernel/btf/vmlinux`
// rather than copied from an upstream header: a prototype that disagrees with
// the kernel is rejected at load, which is the point of declaring them here.
s32 scx_bpf_create_dsq(u64 dsq_id, s32 node) __ksym;
void scx_bpf_dsq_insert(struct task_struct *p, u64 dsq_id, u64 slice, u64 enq_flags) __ksym;
bool scx_bpf_dsq_move_to_local(u64 dsq_id) __ksym;
s32 scx_bpf_select_cpu_dfl(struct task_struct *p, s32 prev_cpu, u64 wake_flags,
			   bool *is_idle) __ksym;

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, AEGIS_CAKE_TASKS);
	__type(key, __s32);
	__type(value, struct aegis_cake_task_ctx);
} task_ctx_map SEC(".maps");

// One row, holding how many tiers `dispatch` may walk. It exists so that the
// loop bound below is a value the verifier does not know, which is what makes
// the clamp load-bearing and the E19-2 negative case meaningful.
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(max_entries, 1);
	__type(key, __u32);
	__type(value, __u32);
} cake_tier_budget SEC(".maps");

__u32 aegis_cake_exit_kind;
__s32 aegis_cake_last_dispatch_cpu;

static __always_inline __u32 aegis_cake_tier_of(__u64 ewma_burst_ns)
{
	if (ewma_burst_ns < AEGIS_CAKE_BURST_CRITICAL_NS_DECLARED) {
		return AEGIS_CAKE_TIER_CRITICAL;
	}
	if (ewma_burst_ns < AEGIS_CAKE_BURST_INTERACTIVE_NS_DECLARED) {
		return AEGIS_CAKE_TIER_INTERACTIVE;
	}
	if (ewma_burst_ns < AEGIS_CAKE_BURST_FRAME_NS_DECLARED) {
		return AEGIS_CAKE_TIER_FRAME;
	}
	return AEGIS_CAKE_TIER_BULK;
}

static __always_inline __u32 aegis_cake_budget(void)
{
	__u32 key = 0;
	__u32 *row = bpf_map_lookup_elem(&cake_tier_budget, &key);

	if (row == NULL) {
		return AEGIS_CAKE_TIERS;
	}
	return *row;
}

SEC("struct_ops/aegis_cake_select_cpu")
s32 BPF_PROG(aegis_cake_select_cpu, struct task_struct *p, s32 prev_cpu, u64 wake_flags)
{
	bool is_idle = false;

	return scx_bpf_select_cpu_dfl(p, prev_cpu, wake_flags, &is_idle);
}

SEC("struct_ops/aegis_cake_enqueue")
void BPF_PROG(aegis_cake_enqueue, struct task_struct *p, u64 enq_flags)
{
	__s32 pid = p->pid;
	struct aegis_cake_task_ctx *tctx = bpf_map_lookup_elem(&task_ctx_map, &pid);
	__u32 tier = AEGIS_CAKE_TIER_INTERACTIVE;

	if (tctx != NULL) {
		tier = tctx->tier & (AEGIS_CAKE_TIERS - 1u);
	}
	scx_bpf_dsq_insert(p, AEGIS_CAKE_DSQ_BASE + tier, SCX_SLICE_DFL, enq_flags);
}

SEC("struct_ops/aegis_cake_dispatch")
void BPF_PROG(aegis_cake_dispatch, s32 cpu, struct task_struct *prev)
{
	__u32 tiers = aegis_cake_budget();
	__u32 index;

	// `prev` is not re-enqueued: the built-in path already handles it, and a
	// fixture that is never attached must not invent a policy for it.
	(void)prev;
	aegis_cake_last_dispatch_cpu = cpu;

	// AEGIS-MUTATE-BEGIN dispatch-loop-bound
	// `tiers` comes from a map value, so the verifier knows nothing about its
	// range. This clamp is the only bound on the loop below. Removing it is the
	// E19-2 negative case: the loop is then unbounded and the object is
	// rejected at load. The gate deletes exactly the lines between this marker
	// and its END.
	if (tiers > AEGIS_CAKE_TIERS) {
		tiers = AEGIS_CAKE_TIERS;
	}
	// AEGIS-MUTATE-END dispatch-loop-bound

	for (index = 0; index < tiers; index++) {
		if (scx_bpf_dsq_move_to_local(AEGIS_CAKE_DSQ_BASE + index)) {
			return;
		}
	}
}

SEC("struct_ops/aegis_cake_running")
void BPF_PROG(aegis_cake_running, struct task_struct *p)
{
	__s32 pid = p->pid;
	struct aegis_cake_task_ctx *tctx = bpf_map_lookup_elem(&task_ctx_map, &pid);
	struct aegis_cake_task_ctx fresh = {};

	if (tctx != NULL) {
		tctx->run_start_ns = bpf_ktime_get_ns();
		return;
	}
	fresh.run_start_ns = bpf_ktime_get_ns();
	fresh.tier = AEGIS_CAKE_TIER_INTERACTIVE;
	bpf_map_update_elem(&task_ctx_map, &pid, &fresh, BPF_ANY);
}

SEC("struct_ops/aegis_cake_stopping")
void BPF_PROG(aegis_cake_stopping, struct task_struct *p, bool runnable)
{
	__s32 pid = p->pid;
	struct aegis_cake_task_ctx *tctx = bpf_map_lookup_elem(&task_ctx_map, &pid);
	__u64 now = bpf_ktime_get_ns();
	__u64 delta;

	(void)runnable;
	if (tctx == NULL || tctx->run_start_ns == 0 || now <= tctx->run_start_ns) {
		return;
	}
	delta = now - tctx->run_start_ns;
	if (tctx->ewma_burst_ns == 0) {
		tctx->ewma_burst_ns = delta;
	} else {
		tctx->ewma_burst_ns = (tctx->ewma_burst_ns / AEGIS_CAKE_EWMA_DIVISOR) *
					     AEGIS_CAKE_EWMA_WEIGHT_OLD +
				     (delta / AEGIS_CAKE_EWMA_DIVISOR) * AEGIS_CAKE_EWMA_WEIGHT_NEW;
	}
	tctx->tier = aegis_cake_tier_of(tctx->ewma_burst_ns);
	tctx->run_start_ns = 0;
}

// `.s` marks the program sleepable. It is not decoration: `scx_bpf_create_dsq`
// is a sleepable kfunc, and on the reference kernel a non-sleepable caller is
// refused at load with "program must be sleepable to call sleepable kfunc
// scx_bpf_create_dsq". That refusal was observed before this suffix was added.
SEC("struct_ops.s/aegis_cake_init")
s32 BPF_PROG(aegis_cake_init)
{
	__u32 index;
	s32 created;

	for (index = 0; index < AEGIS_CAKE_TIERS; index++) {
		created = scx_bpf_create_dsq(AEGIS_CAKE_DSQ_BASE + index, -1);
		if (created != 0) {
			return created;
		}
	}
	return 0;
}

SEC("struct_ops/aegis_cake_exit")
void BPF_PROG(aegis_cake_exit, struct scx_exit_info *ei)
{
	aegis_cake_exit_kind = (__u32)ei->kind;
}

SEC(".struct_ops.link")
struct sched_ext_ops aegis_cake_ops = {
	.select_cpu = (void *)aegis_cake_select_cpu,
	.enqueue = (void *)aegis_cake_enqueue,
	.dispatch = (void *)aegis_cake_dispatch,
	.running = (void *)aegis_cake_running,
	.stopping = (void *)aegis_cake_stopping,
	.init = (void *)aegis_cake_init,
	.exit = (void *)aegis_cake_exit,
	.name = "aegis_cake",
};

// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// P07 boundary fixture: a sched_ext struct_ops in which every handler the object
// declares is a stub. This is the ONLY object milestone M19 attaches to the host
// kernel, and the three properties that make attaching it recoverable are all
// declared right here rather than assumed:
//
//  1. SCX_OPS_SWITCH_PARTIAL. With this flag set, sched_ext schedules only tasks
//     whose policy is SCHED_EXT; SCHED_NORMAL, SCHED_BATCH and SCHED_IDLE tasks
//     stay on the fair-class scheduler. Nothing on the reference profile sets
//     SCHED_EXT, so no task is ever enqueued and the empty handlers below are
//     never called. Without this flag an empty `enqueue` would stall every
//     runnable task on the machine -- the stub is safe because of the flag, not
//     because a handler that does nothing is harmless.
//  2. `timeout_ms`. sched_ext runs a watchdog and ejects a scheduler that leaves
//     a runnable task un-run past this bound, reverting every task to the
//     fair-class scheduler. The bound is declared, not defaulted, so the
//     recovery time is a recorded number.
//  3. The handlers are empty, not absent. The kernel documents `ops.name` as the
//     only mandatory field, so an ops struct with no handlers at all would also
//     load; declaring them and leaving them empty is what "all handlers stubbed"
//     means, and it is what puts each one through the verifier.
//
// The set of handlers declared here is exactly the set listed below and nothing
// wider. `tools/verify_bpf_objects.py` reads the set back from the built object
// and fails if it differs, so this comment cannot drift away from the object.

#include "vmlinux.h"

#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

#include "aegis_bpf_abi.h"

char LICENSE[] SEC("license") = "GPL";

SEC("struct_ops/aegis_stub_select_cpu")
s32 BPF_PROG(aegis_stub_select_cpu, struct task_struct *p, s32 prev_cpu, u64 wake_flags)
{
	(void)p;
	(void)wake_flags;
	return prev_cpu;
}

SEC("struct_ops/aegis_stub_enqueue")
void BPF_PROG(aegis_stub_enqueue, struct task_struct *p, u64 enq_flags)
{
	(void)p;
	(void)enq_flags;
}

SEC("struct_ops/aegis_stub_dequeue")
void BPF_PROG(aegis_stub_dequeue, struct task_struct *p, u64 deq_flags)
{
	(void)p;
	(void)deq_flags;
}

SEC("struct_ops/aegis_stub_dispatch")
void BPF_PROG(aegis_stub_dispatch, s32 cpu, struct task_struct *prev)
{
	(void)cpu;
	(void)prev;
}

SEC("struct_ops/aegis_stub_running")
void BPF_PROG(aegis_stub_running, struct task_struct *p)
{
	(void)p;
}

SEC("struct_ops/aegis_stub_stopping")
void BPF_PROG(aegis_stub_stopping, struct task_struct *p, bool runnable)
{
	(void)p;
	(void)runnable;
}

SEC("struct_ops/aegis_stub_init_task")
s32 BPF_PROG(aegis_stub_init_task, struct task_struct *p, struct scx_init_task_args *args)
{
	(void)p;
	(void)args;
	return 0;
}

SEC("struct_ops/aegis_stub_init")
s32 BPF_PROG(aegis_stub_init)
{
	return 0;
}

SEC("struct_ops/aegis_stub_exit")
void BPF_PROG(aegis_stub_exit, struct scx_exit_info *ei)
{
	(void)ei;
}

SEC(".struct_ops.link")
struct sched_ext_ops aegis_cake_stub_ops = {
	.select_cpu = (void *)aegis_stub_select_cpu,
	.enqueue = (void *)aegis_stub_enqueue,
	.dequeue = (void *)aegis_stub_dequeue,
	.dispatch = (void *)aegis_stub_dispatch,
	.running = (void *)aegis_stub_running,
	.stopping = (void *)aegis_stub_stopping,
	.init_task = (void *)aegis_stub_init_task,
	.init = (void *)aegis_stub_init,
	.exit = (void *)aegis_stub_exit,
	.flags = SCX_OPS_SWITCH_PARTIAL,
	.timeout_ms = AEGIS_CAKE_STUB_TIMEOUT_MS,
	.name = "aegis_cake_stub",
};

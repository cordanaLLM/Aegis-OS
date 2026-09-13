// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// P06 `action_gate`: the exec observation point, as a BPF LSM program.
//
// What this object does: on every `bprm_check_security` it reserves one record
// in a ring buffer, fills it with the identity of the execing task, and submits
// it. What it deliberately does NOT do: deny anything. The hook runs on every
// exec on the machine, and a fixture whose purpose is to prove that a verifier
// accepts an object has no business holding a veto over process creation. The
// only value it returns on its own is AEGIS_ACTION_ALLOW, which is zero; a
// denial already decided by an earlier LSM is passed through unchanged.
//
// The pre-execution approval gate REQ-P06-05 describes is therefore NOT
// implemented here. This object produces the event a gate would decide on; the
// decision, the daemon and the blocking return path are a later milestone.

#include "vmlinux.h"

#include <bpf/bpf_core_read.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

#include "aegis_bpf_abi.h"

char LICENSE[] SEC("license") = "GPL";

struct {
	__uint(type, BPF_MAP_TYPE_RINGBUF);
	__uint(max_entries, AEGIS_ACTION_RINGBUF_BYTES);
} action_ringbuf SEC(".maps");

SEC("lsm/bprm_check_security")
int BPF_PROG(action_gate_exec, struct linux_binprm *bprm, int ret)
{
	struct aegis_action_event *event;

	if (ret != 0) {
		return ret;
	}

	event = bpf_ringbuf_reserve(&action_ringbuf, sizeof(*event), 0);
	// AEGIS-MUTATE-BEGIN ringbuf-null-check
	// bpf_ringbuf_reserve returns a pointer or NULL, and the verifier tracks
	// that as ringbuf_mem_or_null. Removing this test is the E19-1 negative
	// case: the write below then dereferences the or_null type and the object
	// is rejected at load. The gate deletes exactly the lines between this
	// marker and its END, so the negative object differs from this one in this
	// check and nothing else.
	if (event == NULL) {
		return AEGIS_ACTION_ALLOW;
	}
	// AEGIS-MUTATE-END ringbuf-null-check

	event->pid_tgid = bpf_get_current_pid_tgid();
	event->cgroup_id = bpf_get_current_cgroup_id();
	event->kind = AEGIS_ACTION_KIND_EXEC;
	event->verdict = AEGIS_VERDICT_OBSERVED;
	event->argc = (__u32)BPF_CORE_READ(bprm, argc);
	event->reserved = 0;
	bpf_get_current_comm(&event->comm, sizeof(event->comm));
	// The path execve was asked for. `comm` above is still the caller's name at
	// this hook, so the filename is the only field that identifies the program.
	bpf_probe_read_kernel_str(event->filename, sizeof(event->filename),
				  BPF_CORE_READ(bprm, filename));
	bpf_ringbuf_submit(event, 0);

	return AEGIS_ACTION_ALLOW;
}

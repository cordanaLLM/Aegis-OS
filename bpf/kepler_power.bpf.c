// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// P13 `kepler_power`: the per-cgroup accounting probe, as a tracepoint program.
//
// It accumulates scheduler runtime per cgroup on `sched/sched_switch` and
// multiplies that runtime by a declared constant. That product is NOT a power
// or an energy measurement and the field it lands in says so in its name: no
// RAPL MSR, no `/sys/class/powercap` file and no ACPI meter is read anywhere in
// this object. Milestone M21 is where a counter is actually read; until then the
// probe proves only that the tracepoint attaches and the accounting arithmetic
// passes the verifier.

#include "vmlinux.h"

#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

#include "aegis_bpf_abi.h"

char LICENSE[] SEC("license") = "GPL";

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, AEGIS_KEPLER_MAX_CGROUPS);
	__type(key, __u64);
	__type(value, struct aegis_power_sample);
} cgroup_power_map SEC(".maps");

static __always_inline int aegis_kepler_start(__u64 cgroup_id, __u64 now)
{
	struct aegis_power_sample fresh = {};

	fresh.cgroup_id = cgroup_id;
	fresh.last_timestamp_ns = now;
	return bpf_map_update_elem(&cgroup_power_map, &cgroup_id, &fresh, BPF_NOEXIST);
}

SEC("tracepoint/sched/sched_switch")
int aegis_kepler_sched_switch(struct trace_event_raw_sched_switch *ctx)
{
	struct aegis_power_sample *sample;
	__u64 now = bpf_ktime_get_ns();
	__u64 cgroup_id = bpf_get_current_cgroup_id();
	__u64 delta_ns;
	__u32 slot;

	sample = bpf_map_lookup_elem(&cgroup_power_map, &cgroup_id);
	if (sample == NULL) {
		aegis_kepler_start(cgroup_id, now);
		return 0;
	}

	if (now <= sample->last_timestamp_ns) {
		return 0;
	}
	delta_ns = now - sample->last_timestamp_ns;
	sample->last_timestamp_ns = now;

	// The slot is derived from the incoming task and is therefore an unknown
	// value as far as the verifier is concerned.
	slot = (__u32)ctx->next_pid;
	// AEGIS-MUTATE-BEGIN map-value-index-bound
	// This mask is the only thing that keeps the index inside the map value.
	// Removing it is the E19-3 negative case: `runtime_ns[slot]` then reaches
	// past value_size and the object is rejected at load. The gate deletes
	// exactly the lines between this marker and its END.
	slot &= AEGIS_KEPLER_SLOT_MASK;
	// AEGIS-MUTATE-END map-value-index-bound
	sample->runtime_ns[slot] += delta_ns;

	// Runtime multiplied by a declared constant. Recorded, never a measurement.
	sample->pseudo_energy_uj_unmeasured +=
		(delta_ns / 1000000ULL) * AEGIS_KEPLER_TDP_MILLIWATT_UNMEASURED / 1000ULL;
	return 0;
}

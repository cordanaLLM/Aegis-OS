// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2
//
// The M19 verifier probe: it puts one eBPF object through the kernel verifier
// and retains the verifier's own log, for a load that succeeds exactly as for
// one that fails.
//
// Why this exists rather than a `bpftool prog load`. Three of the milestone's
// requirements do not fit a one-shot command:
//
//  * the verifier log must be retained for EVERY load, positive and negative,
//    and per program rather than per object. libbpf writes into one shared
//    buffer unless each program is given its own, so this probe hands every
//    program a private buffer with log_level 1 before the object is loaded;
//  * the P06 acceptance needs an exec event to reach a ring-buffer consumer,
//    which is a poll loop, not a command;
//  * the P07 boundary case must attach a struct_ops link and then detach it
//    from the same process, with the kernel's own `root/ops` read while the
//    link is held.
//
// Bounds, because this runs against a live kernel. Every mode arms a SIGALRM
// deadline and exits AEGIS_EXIT_DEADLINE rather than blocking; every loop has a
// scalar upper bound; the attach window is a caller-supplied millisecond count
// and the link is destroyed on every exit path out of that window.

#include <ctype.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#include <linux/types.h>

#include <bpf/bpf.h>
#include <bpf/libbpf.h>

#include "aegis_bpf_abi.h"

#define AEGIS_EXIT_OK 0
#define AEGIS_EXIT_LOAD_FAILED 1
#define AEGIS_EXIT_ATTACH_FAILED 2
#define AEGIS_EXIT_NO_EVENT 3
#define AEGIS_EXIT_USAGE 4
#define AEGIS_EXIT_DEADLINE 5
// A log that could not be WRITTEN is not a load the verifier REJECTED, and the
// two must never share an exit code. They did: write_logs() failing returned
// -1, main() turned that into AEGIS_EXIT_LOAD_FAILED, and a caller reading the
// log off disk after a non-zero exit then accepted whatever file was already
// there as this run's evidence.
#define AEGIS_EXIT_LOG_FAILED 6

#define AEGIS_MAX_PROGS 32
// Per-program verifier-log buffer. Demand-zero BSS, so the 32 slots cost
// nothing until a program actually logs into one. It is deliberately generous
// but still finite: a rejected unbounded loop traces tens of thousands of
// instructions and can outgrow any fixed buffer, which is why truncation is
// reported below rather than hidden.
#define AEGIS_LOG_BYTES (4 * 1024 * 1024)
#define AEGIS_POLL_SLICE_MS 200
#define AEGIS_MAX_POLLS 50
#define AEGIS_MAX_HOLD_SLICES 200
#define AEGIS_HOLD_SLICE_MS 50
#define AEGIS_SCHED_EXT_DIR "/sys/kernel/sched_ext"
#define AEGIS_SCHED_EXT_OPS_ATTR "root/ops"
#define AEGIS_SYSFS_PATH_BOUND 256
// Upper bound on the caller-supplied run nonce, which is copied into the log
// header verbatim: it is bounded and alphabet-checked here rather than trusted.
#define AEGIS_NONCE_BOUND 128
// The number of top-level counters read below, pinned to its value and checked
// against the table by _Static_assert rather than derived from it.
#define AEGIS_SCHED_EXT_COUNTERS 3
#define AEGIS_MARKER_BASENAME "/aegis_exec_prob"
// Upper bound on the sysfs scheduler-name read. SCX_OPS_NAME_LEN is 128 in the
// running kernel's BTF; this is that bound with room for the trailing newline.
#define AEGIS_OPS_NAME_BOUND 160

// One private verifier-log buffer per program, sized so that a rejection
// message is retained in full rather than truncated into an unusable excerpt.
static char g_logs[AEGIS_MAX_PROGS][AEGIS_LOG_BYTES];
// The counters M19's criterion asks to capture at each point of the takeover.
// They are TOP-LEVEL attributes of /sys/kernel/sched_ext/, NOT members of
// root/: root/ holds exactly `events` and `ops`. switch_all shows the kernel's
// own scx_switching_all, so a scheduler declaring SCX_OPS_SWITCH_PARTIAL reads
// 0 there -- which is the kernel measuring "no task was switched to it".
static const char *const g_sched_ext_counters[AEGIS_SCHED_EXT_COUNTERS] = {
	"switch_all",
	"nr_rejected",
	"enable_seq",
};
_Static_assert(sizeof(g_sched_ext_counters) / sizeof(g_sched_ext_counters[0]) ==
		       AEGIS_SCHED_EXT_COUNTERS,
	       "the counter bound must equal the number of counters read");
static struct bpf_program *g_progs[AEGIS_MAX_PROGS];
static int g_prog_count;
static int g_marker_seen;

struct aegis_options {
	const char *object;
	const char *log_path;
	const char *mode;
	const char *marker;
	const char *nonce;
	unsigned int deadline_s;
	unsigned int hold_ms;
};

static void on_deadline(int signo)
{
	static const char msg[] = "probe=deadline\n";

	(void)signo;
	if (write(STDOUT_FILENO, msg, sizeof(msg) - 1) < 0) {
		_exit(AEGIS_EXIT_DEADLINE);
	}
	_exit(AEGIS_EXIT_DEADLINE);
}

static int quiet_print(enum libbpf_print_level level, const char *format, va_list args)
{
	if (level == LIBBPF_WARN) {
		return vfprintf(stderr, format, args);
	}
	return 0;
}

// Give every program in the object its own verifier-log buffer at log level 1,
// so the log survives both outcomes. Returns -1 if the object carries more
// programs than the fixed table holds: truncating that list would silently drop
// a program's log, which is the one thing this probe must not do.
static int arm_logs(struct bpf_object *obj)
{
	struct bpf_program *prog;

	g_prog_count = 0;
	bpf_object__for_each_program(prog, obj) {
		if (g_prog_count >= AEGIS_MAX_PROGS) {
			fprintf(stderr, "object carries more than %d programs\n", AEGIS_MAX_PROGS);
			return -1;
		}
		g_logs[g_prog_count][0] = '\0';
		if (bpf_program__set_log_level(prog, 1) != 0) {
			return -1;
		}
		if (bpf_program__set_log_buf(prog, g_logs[g_prog_count], AEGIS_LOG_BYTES) != 0) {
			return -1;
		}
		g_progs[g_prog_count] = prog;
		g_prog_count++;
	}
	return 0;
}

// Write every retained log to the evidence file, whether the load succeeded or
// not. A program whose buffer is empty is recorded as empty rather than omitted.
static int write_logs(const char *path, const char *object, const char *nonce, int load_errno)
{
	FILE *out = fopen(path, "we");
	int index;

	if (out == NULL) {
		fprintf(stderr, "cannot write verifier log to %s: %s\n", path, strerror(errno));
		return -1;
	}
	fprintf(out, "# verifier log for %s\n", object);
	// The caller's nonce, so that a log can be told apart from one an earlier
	// run left at the same path. A caller that requires this line requires the
	// log to have been produced by the load it just ran.
	fprintf(out, "# run-nonce %s\n", nonce);
	fprintf(out, "# libbpf runtime %s, load errno %d (%s)\n", libbpf_version_string(),
		load_errno, load_errno == 0 ? "load reported success" : strerror(load_errno));
	if (load_errno == ENOSPC) {
		fprintf(out, "# ENOSPC here means the VERIFIER LOG outgrew this probe's %d-byte\n"
			     "# buffer, not that a disk filled. The kernel's log is a rotating\n"
			     "# buffer, so what is retained is the TAIL, which is where the\n"
			     "# verdict and the instruction count are. A section that was cut\n"
			     "# is marked below and its first line starts mid-token.\n",
			AEGIS_LOG_BYTES);
	}
	fprintf(out, "# programs: %d\n", g_prog_count);
	for (index = 0; index < g_prog_count; index++) {
		fprintf(out, "\n=== program %s (section %s) ===\n",
			bpf_program__name(g_progs[index]),
			bpf_program__section_name(g_progs[index]));
		if (g_logs[index][0] == '\0') {
			fprintf(out, "(the kernel returned no verifier log for this program)\n");
			continue;
		}
		if (strlen(g_logs[index]) >= AEGIS_LOG_BYTES - 1) {
			fprintf(out, "(TRUNCATED: this program's log filled the buffer; the "
				     "kernel retained the tail)\n");
		}
		fputs(g_logs[index], out);
		if (g_logs[index][strlen(g_logs[index]) - 1] != '\n') {
			fputc('\n', out);
		}
	}
	return fclose(out) == 0 ? 0 : -1;
}

static void report_programs(void)
{
	int index;

	for (index = 0; index < g_prog_count; index++) {
		printf("program=%s section=%s type=%d fd_valid=%d\n",
		       bpf_program__name(g_progs[index]),
		       bpf_program__section_name(g_progs[index]),
		       (int)bpf_program__type(g_progs[index]),
		       bpf_program__fd(g_progs[index]) >= 0 ? 1 : 0);
	}
}

static struct bpf_map *find_map(struct bpf_object *obj, enum bpf_map_type type)
{
	struct bpf_map *map;

	bpf_object__for_each_map(map, obj) {
		if (bpf_map__type(map) == type) {
			return map;
		}
	}
	return NULL;
}

static struct bpf_program *find_program(struct bpf_object *obj, const char *section_prefix)
{
	struct bpf_program *prog;
	size_t want = strlen(section_prefix);

	bpf_object__for_each_program(prog, obj) {
		if (strncmp(bpf_program__section_name(prog), section_prefix, want) == 0) {
			return prog;
		}
	}
	return NULL;
}

// The marker is identified by the path execve was asked for, compared as a
// suffix so that a truncated path fails the comparison instead of passing it.
static int ends_with(const char *text, const char *suffix)
{
	size_t text_len = strnlen(text, AEGIS_ACTION_PATH_BYTES);
	size_t suffix_len = strlen(suffix);

	if (suffix_len > text_len) {
		return 0;
	}
	return strncmp(text + text_len - suffix_len, suffix, suffix_len) == 0;
}

static int on_action_event(void *ctx, void *data, size_t size)
{
	const struct aegis_action_event *event = data;

	(void)ctx;
	if (size != sizeof(*event)) {
		return 0;
	}
	if (ends_with(event->filename, AEGIS_MARKER_BASENAME)) {
		g_marker_seen = 1;
		printf("event filename=%s comm=%s kind=%u verdict=%u argc=%u cgroup_id=%llu\n",
		       event->filename, event->comm, event->kind, event->verdict, event->argc,
		       (unsigned long long)event->cgroup_id);
	}
	return 0;
}

// Run the marker binary once, so a known exec passes through the attached hook.
static int run_marker(const char *marker)
{
	char *const argv[] = { (char *)(uintptr_t)marker, NULL };
	pid_t child = fork();
	int status = 0;

	if (child < 0) {
		return -1;
	}
	if (child == 0) {
		execv(marker, argv);
		_exit(127);
	}
	if (waitpid(child, &status, 0) != child) {
		return -1;
	}
	return 0;
}

// Read one attribute under /sys/kernel/sched_ext/, relative name and all.
static int read_sched_ext_attr(const char *name, char *value, size_t size)
{
	char path[AEGIS_SYSFS_PATH_BOUND];
	FILE *in;
	size_t got;

	if (snprintf(path, sizeof(path), "%s/%s", AEGIS_SCHED_EXT_DIR, name) >=
	    (int)sizeof(path)) {
		return -1;
	}
	in = fopen(path, "re");
	if (in == NULL) {
		return -1;
	}
	got = fread(value, 1, size - 1, in);
	value[got] = '\0';
	fclose(in);
	value[strcspn(value, "\n")] = '\0';
	return 0;
}

static int print_sched_ext_ops(const char *label)
{
	char value[AEGIS_OPS_NAME_BOUND];

	if (read_sched_ext_attr(AEGIS_SCHED_EXT_OPS_ATTR, value, sizeof(value)) != 0) {
		printf("sched_ext_ops_%s=<absent: %s>\n", label, strerror(errno));
		return -1;
	}
	printf("sched_ext_ops_%s=%s\n", label, value);
	return 0;
}

// The top-level counters at one point of the sequence. They are read in THIS
// process, the one holding the struct_ops link, because a reading taken outside
// the hold window is not a reading taken during it.
static void print_sched_ext_counters(const char *label)
{
	char value[AEGIS_OPS_NAME_BOUND];
	int index;

	for (index = 0; index < AEGIS_SCHED_EXT_COUNTERS; index++) {
		if (read_sched_ext_attr(g_sched_ext_counters[index], value, sizeof(value)) !=
		    0) {
			printf("sched_ext_%s_%s=<absent>\n", g_sched_ext_counters[index], label);
			continue;
		}
		printf("sched_ext_%s_%s=%s\n", g_sched_ext_counters[index], label, value);
	}
}

static int mode_lsm_probe(struct bpf_object *obj, const struct aegis_options *options)
{
	struct bpf_program *prog = find_program(obj, "lsm/");
	struct bpf_map *map = find_map(obj, BPF_MAP_TYPE_RINGBUF);
	struct ring_buffer *rb;
	struct bpf_link *link;
	int polls;

	if (prog == NULL || map == NULL) {
		fprintf(stderr, "object carries no lsm program or no ring buffer\n");
		return AEGIS_EXIT_ATTACH_FAILED;
	}
	rb = ring_buffer__new(bpf_map__fd(map), on_action_event, NULL, NULL);
	if (rb == NULL) {
		fprintf(stderr, "ring_buffer__new failed: %s\n", strerror(errno));
		return AEGIS_EXIT_ATTACH_FAILED;
	}
	link = bpf_program__attach(prog);
	if (link == NULL) {
		fprintf(stderr, "lsm attach failed: %s\n", strerror(errno));
		ring_buffer__free(rb);
		return AEGIS_EXIT_ATTACH_FAILED;
	}
	printf("attached=lsm ringbuf=%s\n", bpf_map__name(map));
	if (run_marker(options->marker) != 0) {
		fprintf(stderr, "marker exec failed: %s\n", strerror(errno));
	}
	for (polls = 0; polls < AEGIS_MAX_POLLS && g_marker_seen == 0; polls++) {
		if (ring_buffer__poll(rb, AEGIS_POLL_SLICE_MS) < 0) {
			break;
		}
	}
	bpf_link__destroy(link);
	ring_buffer__free(rb);
	printf("detached=lsm marker_seen=%d\n", g_marker_seen);
	return g_marker_seen ? AEGIS_EXIT_OK : AEGIS_EXIT_NO_EVENT;
}

static int mode_sops_attach(struct bpf_object *obj, const struct aegis_options *options)
{
	struct bpf_map *map = find_map(obj, BPF_MAP_TYPE_STRUCT_OPS);
	struct bpf_link *link;
	unsigned int slices = options->hold_ms / AEGIS_HOLD_SLICE_MS;
	unsigned int slice;

	if (map == NULL) {
		fprintf(stderr, "object carries no struct_ops map\n");
		return AEGIS_EXIT_ATTACH_FAILED;
	}
	print_sched_ext_ops("before");
	print_sched_ext_counters("before");
	link = bpf_map__attach_struct_ops(map);
	if (link == NULL) {
		fprintf(stderr, "struct_ops attach failed: %s\n", strerror(errno));
		print_sched_ext_ops("after_failed_attach");
		return AEGIS_EXIT_ATTACH_FAILED;
	}
	printf("attached=struct_ops map=%s\n", bpf_map__name(map));
	print_sched_ext_ops("during");
	print_sched_ext_counters("during");
	if (slices > AEGIS_MAX_HOLD_SLICES) {
		slices = AEGIS_MAX_HOLD_SLICES;
	}
	for (slice = 0; slice < slices; slice++) {
		usleep(AEGIS_HOLD_SLICE_MS * 1000);
	}
	print_sched_ext_ops("still_attached");
	print_sched_ext_counters("still_attached");
	printf("detach_rc=%d\n", bpf_link__destroy(link));
	print_sched_ext_ops("after");
	print_sched_ext_counters("after");
	return AEGIS_EXIT_OK;
}

static int mode_sops_load(struct bpf_object *obj)
{
	struct bpf_map *map = find_map(obj, BPF_MAP_TYPE_STRUCT_OPS);

	if (map == NULL) {
		fprintf(stderr, "object carries no struct_ops map\n");
		return AEGIS_EXIT_LOAD_FAILED;
	}
	// The map exists and has a file descriptor: the kernel accepted the
	// struct_ops value, including every program slot in it. No link is created,
	// so no scheduler is enabled by this mode.
	printf("struct_ops_map=%s fd_valid=%d\n", bpf_map__name(map),
	       bpf_map__fd(map) >= 0 ? 1 : 0);
	return AEGIS_EXIT_OK;
}

static int dispatch(struct bpf_object *obj, const struct aegis_options *options)
{
	if (strcmp(options->mode, "load") == 0) {
		return AEGIS_EXIT_OK;
	}
	if (strcmp(options->mode, "lsm-probe") == 0) {
		return mode_lsm_probe(obj, options);
	}
	if (strcmp(options->mode, "sops-load") == 0) {
		return mode_sops_load(obj);
	}
	if (strcmp(options->mode, "sops-attach") == 0) {
		return mode_sops_attach(obj, options);
	}
	fprintf(stderr, "unknown mode %s\n", options->mode);
	return AEGIS_EXIT_USAGE;
}

static int parse_option(struct aegis_options *options, const char *name, const char *value)
{
	if (strcmp(name, "--object") == 0) {
		options->object = value;
	} else if (strcmp(name, "--log") == 0) {
		options->log_path = value;
	} else if (strcmp(name, "--mode") == 0) {
		options->mode = value;
	} else if (strcmp(name, "--marker") == 0) {
		options->marker = value;
	} else if (strcmp(name, "--nonce") == 0) {
		options->nonce = value;
	} else if (strcmp(name, "--deadline") == 0) {
		options->deadline_s = (unsigned int)strtoul(value, NULL, 10);
	} else if (strcmp(name, "--hold-ms") == 0) {
		options->hold_ms = (unsigned int)strtoul(value, NULL, 10);
	} else {
		return -1;
	}
	return 0;
}

// The nonce goes into the log header, so it is bounded and restricted to an
// alphabet that cannot forge a header line of its own.
static int valid_nonce(const char *nonce)
{
	size_t length = strnlen(nonce, AEGIS_NONCE_BOUND);
	size_t index;

	if (length == 0 || length >= AEGIS_NONCE_BOUND) {
		return 0;
	}
	for (index = 0; index < length; index++) {
		if (!isalnum((unsigned char)nonce[index]) && nonce[index] != '-') {
			return 0;
		}
	}
	return 1;
}

static int parse(int argc, char **argv, struct aegis_options *options)
{
	int index;

	options->deadline_s = 30;
	options->hold_ms = 0;
	for (index = 1; index + 1 < argc; index += 2) {
		if (parse_option(options, argv[index], argv[index + 1]) != 0) {
			return -1;
		}
	}
	if (index != argc || options->object == NULL || options->log_path == NULL ||
	    options->mode == NULL || options->nonce == NULL) {
		return -1;
	}
	return valid_nonce(options->nonce) ? 0 : -1;
}

// Returns the exit code this load earns, never a bare -1: a log that could not
// be written exits AEGIS_EXIT_LOG_FAILED and a rejected load exits
// AEGIS_EXIT_LOAD_FAILED, so the caller can tell the two apart.
static int load_object(const struct aegis_options *options, struct bpf_object **out)
{
	struct bpf_object *obj = bpf_object__open_file(options->object, NULL);
	int rc;

	if (obj == NULL) {
		fprintf(stderr, "open failed: %s\n", strerror(errno));
		return AEGIS_EXIT_LOAD_FAILED;
	}
	if (arm_logs(obj) != 0) {
		bpf_object__close(obj);
		return AEGIS_EXIT_LOAD_FAILED;
	}
	rc = bpf_object__load(obj);
	if (write_logs(options->log_path, options->object, options->nonce, rc == 0 ? 0 : -rc) !=
	    0) {
		bpf_object__close(obj);
		return AEGIS_EXIT_LOG_FAILED;
	}
	report_programs();
	printf("load_rc=%d\n", rc);
	if (rc != 0) {
		bpf_object__close(obj);
		return AEGIS_EXIT_LOAD_FAILED;
	}
	*out = obj;
	return AEGIS_EXIT_OK;
}

int main(int argc, char **argv)
{
	struct aegis_options options = { 0 };
	struct bpf_object *obj = NULL;
	int rc;

	if (parse(argc, argv, &options) != 0) {
		fprintf(stderr,
			"usage: %s --object PATH --log PATH --mode "
			"load|lsm-probe|sops-load|sops-attach --nonce ALNUM-OR-DASH "
			"[--marker PATH] [--deadline S] [--hold-ms MS]\n",
			argv[0]);
		return AEGIS_EXIT_USAGE;
	}
	// Line buffering, so that everything printed before a deadline _exit() has
	// already reached the caller. A block-buffered stdout lost the whole run.
	setvbuf(stdout, NULL, _IOLBF, 0);
	signal(SIGALRM, on_deadline);
	alarm(options.deadline_s);
	libbpf_set_print(quiet_print);
	printf("libbpf_runtime=%s\n", libbpf_version_string());
	rc = load_object(&options, &obj);
	if (rc != AEGIS_EXIT_OK) {
		return rc;
	}
	rc = dispatch(obj, &options);
	bpf_object__close(obj);
	return rc;
}

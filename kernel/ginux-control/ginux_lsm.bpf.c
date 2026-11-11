// SPDX-License-Identifier: GPL-2.0-only
/* Gate 16 BPF-LSM feasibility prototype. */

typedef unsigned int __u32;
typedef unsigned long long __u64;

#define SEC(name) __attribute__((section(name), used))
#define __uint(name, value) int (*name)[value]
#define __type(name, value) value *name
#define BPF_MAP_TYPE_HASH 1
#define BPF_ANY 0
#define EPERM 1

static void *(*bpf_map_lookup_elem)(void *map, const void *key) =
	(void *)1;
static __u64 (*bpf_get_current_cgroup_id)(void) = (void *)80;

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 64);
	__type(key, __u64);
	__type(value, __u32);
} blocked_cgroups SEC(".maps");

SEC("lsm/path_mkdir")
int ginux_path_mkdir(__u64 *context)
{
	__u64 cgroup_id;
	int prior_result = (int)context[3];

	if (prior_result != 0)
		return prior_result;
	cgroup_id = bpf_get_current_cgroup_id();
	if (bpf_map_lookup_elem(&blocked_cgroups, &cgroup_id) == 0)
		return 0;
	return -EPERM;
}

char ginux_license[] SEC("license") = "GPL";

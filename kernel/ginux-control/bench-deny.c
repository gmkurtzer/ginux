// SPDX-License-Identifier: GPL-2.0-only

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/audit.h>
#include <linux/filter.h>
#include <linux/seccomp.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

static uint64_t nanoseconds(struct timespec value)
{
	return (uint64_t)value.tv_sec * 1000000000ULL +
	       (uint64_t)value.tv_nsec;
}

static int install_seccomp(void)
{
	struct sock_filter instructions[] = {
		BPF_STMT(BPF_LD | BPF_W | BPF_ABS,
			 offsetof(struct seccomp_data, arch)),
		BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, AUDIT_ARCH_X86_64, 1, 0),
		BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS),
		BPF_STMT(BPF_LD | BPF_W | BPF_ABS,
			 offsetof(struct seccomp_data, nr)),
		BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, SYS_mkdirat, 0, 1),
		BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | EPERM),
		BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
	};
	struct sock_fprog program = {
		.len = (unsigned short)(sizeof(instructions) / sizeof(instructions[0])),
		.filter = instructions,
	};

	if (prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0)
		return -1;
	return (int)syscall(SYS_seccomp, SECCOMP_SET_MODE_FILTER, 0, &program);
}

int main(int argc, char **argv)
{
	struct timespec start;
	struct timespec finish;
	unsigned long iterations;
	unsigned long index;
	double elapsed;

	if (argc != 3) {
		fprintf(stderr, "usage: %s ITERATIONS external|seccomp\n", argv[0]);
		return 2;
	}
	errno = 0;
	iterations = strtoul(argv[1], NULL, 10);
	if (errno != 0 || iterations == 0)
		return 2;
	if (strcmp(argv[2], "seccomp") == 0) {
		if (install_seccomp() != 0)
			return 1;
	} else if (strcmp(argv[2], "external") != 0) {
		return 2;
	}
	if (clock_gettime(CLOCK_MONOTONIC_RAW, &start) != 0)
		return 1;
	for (index = 0; index < iterations; ++index) {
		errno = 0;
		if (syscall(SYS_mkdirat, AT_FDCWD,
			    "/tmp/ginux-deny-benchmark", 0700) != -1 ||
		    errno != EPERM)
			return 1;
	}
	if (clock_gettime(CLOCK_MONOTONIC_RAW, &finish) != 0)
		return 1;
	elapsed = (double)(nanoseconds(finish) - nanoseconds(start)) /
		  1000000000.0;
	printf("iterations=%lu seconds=%.9f denials_per_second=%.3f\n",
	       iterations, elapsed, (double)iterations / elapsed);
	return 0;
}

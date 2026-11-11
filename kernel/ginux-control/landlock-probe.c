// SPDX-License-Identifier: GPL-2.0-only

#define _GNU_SOURCE
#include <errno.h>
#include <linux/landlock.h>
#include <stdio.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

int main(void)
{
	long abi = syscall(SYS_landlock_create_ruleset, NULL, 0,
			   LANDLOCK_CREATE_RULESET_VERSION);

	if (abi < 0) {
		fprintf(stderr, "landlock unavailable: %s\n", strerror(errno));
		return 1;
	}
	printf("GINUX_LANDLOCK: abi=%ld\n", abi);
	return 0;
}

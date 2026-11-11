// SPDX-License-Identifier: GPL-2.0-only

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

int main(int argc, char **argv)
{
	long result;

	if (argc != 2) {
		fprintf(stderr, "usage: %s PATH\n", argv[0]);
		return 2;
	}
	result = syscall(SYS_mkdirat, AT_FDCWD, argv[1], 0700);
	printf("result=%ld errno=%d (%s)\n", result, errno, strerror(errno));
	return result == -1 ? errno : 0;
}

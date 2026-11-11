// SPDX-License-Identifier: GPL-2.0-only

#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

static uint64_t nanoseconds(struct timespec value)
{
	return (uint64_t)value.tv_sec * 1000000000ULL +
	       (uint64_t)value.tv_nsec;
}

int main(int argc, char **argv)
{
	struct timespec start;
	struct timespec finish;
	unsigned long iterations;
	unsigned long index;
	double elapsed;
	char path[128];

	if (argc != 2) {
		fprintf(stderr, "usage: %s ITERATIONS\n", argv[0]);
		return 2;
	}
	errno = 0;
	iterations = strtoul(argv[1], NULL, 10);
	if (errno != 0 || iterations == 0)
		return 2;
	if (clock_gettime(CLOCK_MONOTONIC_RAW, &start) != 0)
		return 1;
	for (index = 0; index < iterations; ++index) {
		int length = snprintf(path, sizeof(path),
				      "/tmp/ginux-mkdir-%ld-%lu",
				      (long)getpid(), index);
		if (length < 0 || (size_t)length >= sizeof(path))
			return 1;
		if (mkdirat(AT_FDCWD, path, 0700) != 0)
			return 1;
		if (unlinkat(AT_FDCWD, path, AT_REMOVEDIR) != 0)
			return 1;
	}
	if (clock_gettime(CLOCK_MONOTONIC_RAW, &finish) != 0)
		return 1;
	elapsed = (double)(nanoseconds(finish) - nanoseconds(start)) /
		  1000000000.0;
	printf("iterations=%lu seconds=%.9f operations_per_second=%.3f\n",
	       iterations, elapsed, (double)iterations / elapsed);
	return 0;
}

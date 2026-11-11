// SPDX-License-Identifier: GPL-2.0-only

#include <errno.h>
#include <sched.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

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

	if (argc != 2) {
		fprintf(stderr, "usage: %s ITERATIONS\n", argv[0]);
		return 2;
	}
	errno = 0;
	iterations = strtoul(argv[1], NULL, 10);
	if (errno != 0 || iterations == 0) {
		fprintf(stderr, "invalid iteration count\n");
		return 2;
	}
	if (clock_gettime(CLOCK_MONOTONIC_RAW, &start) != 0)
		return 1;
	for (index = 0; index < iterations; ++index) {
		if (sched_yield() != 0)
			return 1;
	}
	if (clock_gettime(CLOCK_MONOTONIC_RAW, &finish) != 0)
		return 1;
	elapsed = (double)(nanoseconds(finish) - nanoseconds(start)) /
		  1000000000.0;
	printf("iterations=%lu seconds=%.9f events_per_second=%.3f\n",
	       iterations, elapsed, (double)iterations / elapsed);
	return 0;
}

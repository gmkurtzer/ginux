#define _GNU_SOURCE
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

enum { ITERATIONS = 20000 };

static uint64_t nanoseconds(void) {
    struct timespec value;
    if (clock_gettime(CLOCK_MONOTONIC, &value) != 0) abort();
    return (uint64_t)value.tv_sec * 1000000000ULL + (uint64_t)value.tv_nsec;
}

static void *worker(void *argument) {
    uintptr_t id = (uintptr_t)argument;
    for (size_t i = 0; i < ITERATIONS; ++i) {
        size_t size = 16 + ((i * 73 + id * 31) % 4096);
        unsigned char *memory = malloc(size);
        if (memory == NULL) return (void *)1;
        memory[0] = (unsigned char)i;
        memory[size - 1] = (unsigned char)id;
        free(memory);
    }
    return NULL;
}

int main(int argc, char **argv) {
    size_t count = argc > 1 ? (size_t)strtoul(argv[1], NULL, 10) : 1;
    if (count == 0 || count > 16) return 2;
    pthread_t threads[16];
    uint64_t start = nanoseconds();
    for (uintptr_t i = 0; i < count; ++i)
        if (pthread_create(&threads[i], NULL, worker, (void *)i) != 0) return 3;
    for (size_t i = 0; i < count; ++i) {
        void *result = NULL;
        if (pthread_join(threads[i], &result) != 0 || result != NULL) return 4;
    }
    uint64_t elapsed = nanoseconds() - start;
    printf("threads=%zu operations=%zu elapsed_ns=%llu ops_per_second=%.2f\n",
           count, count * (size_t)ITERATIONS, (unsigned long long)elapsed,
           (double)(count * (size_t)ITERATIONS) * 1000000000.0 / (double)elapsed);
    return 0;
}

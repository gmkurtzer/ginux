#define _GNU_SOURCE
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdatomic.h>
#include <sys/resource.h>
#include <time.h>
#include <unistd.h>

enum { ITERATIONS = 20000, LIVE_SLOTS = 256 };

static pthread_barrier_t ready_barrier;
static pthread_barrier_t start_barrier;
static atomic_ulong live_requested;

static int wait_at(pthread_barrier_t *barrier) {
    int result = pthread_barrier_wait(barrier);
    return result == 0 || result == PTHREAD_BARRIER_SERIAL_THREAD ? 0 : result;
}

static uint64_t nanoseconds(void) {
    struct timespec value;
    if (clock_gettime(CLOCK_MONOTONIC, &value) != 0) abort();
    return (uint64_t)value.tv_sec * 1000000000ULL + (uint64_t)value.tv_nsec;
}

static uint64_t resident_bytes(void) {
    FILE *source = fopen("/proc/self/statm", "r");
    unsigned long pages = 0;
    unsigned long resident = 0;
    if (source == NULL || fscanf(source, "%lu %lu", &pages, &resident) != 2) abort();
    fclose(source);
    (void)pages;
    return (uint64_t)resident * (uint64_t)sysconf(_SC_PAGESIZE);
}

static void *worker(void *argument) {
    uintptr_t id = (uintptr_t)argument;
    unsigned char *slots[LIVE_SLOTS];
    size_t sizes[LIVE_SLOTS];
    size_t requested = 0;
    for (size_t i = 0; i < LIVE_SLOTS; ++i) {
        sizes[i] = 16 + ((i * 73 + id * 31) % 4096);
        slots[i] = malloc(sizes[i]);
        if (slots[i] == NULL) return (void *)1;
        slots[i][0] = (unsigned char)i;
        slots[i][sizes[i] - 1] = (unsigned char)id;
        requested += sizes[i];
    }
    atomic_fetch_add_explicit(&live_requested, requested, memory_order_relaxed);
    if (wait_at(&ready_barrier) != 0) return (void *)2;
    if (wait_at(&start_barrier) != 0) return (void *)3;
    for (size_t i = 0; i < ITERATIONS; ++i) {
        size_t slot = i % LIVE_SLOTS;
        free(slots[slot]);
        size_t size = 16 + ((i * 73 + id * 31) % 4096);
        slots[slot] = malloc(size);
        if (slots[slot] == NULL) return (void *)4;
        slots[slot][0] = (unsigned char)i;
        slots[slot][size - 1] = (unsigned char)id;
    }
    for (size_t i = 0; i < LIVE_SLOTS; ++i) free(slots[i]);
    return NULL;
}

int main(int argc, char **argv) {
    size_t count = argc > 1 ? (size_t)strtoul(argv[1], NULL, 10) : 1;
    if (count == 0 || count > 16) return 2;
    pthread_t threads[16];
    uint64_t baseline_rss = resident_bytes();
    atomic_init(&live_requested, 0);
    if (pthread_barrier_init(&ready_barrier, NULL, (unsigned)count + 1) != 0 ||
        pthread_barrier_init(&start_barrier, NULL, (unsigned)count + 1) != 0) return 3;
    for (uintptr_t i = 0; i < count; ++i)
        if (pthread_create(&threads[i], NULL, worker, (void *)i) != 0) return 4;
    if (wait_at(&ready_barrier) != 0) return 5;
    uint64_t live_rss = resident_bytes();
    uint64_t requested = atomic_load_explicit(&live_requested, memory_order_relaxed);
    uint64_t start = nanoseconds();
    if (wait_at(&start_barrier) != 0) return 6;
    for (size_t i = 0; i < count; ++i) {
        void *result = NULL;
        if (pthread_join(threads[i], &result) != 0 || result != NULL) return 7;
    }
    uint64_t elapsed = nanoseconds() - start;
    size_t operations = count * (size_t)ITERATIONS;
    uint64_t live_delta = live_rss > baseline_rss ? live_rss - baseline_rss : 0;
    struct rusage usage;
    if (getrusage(RUSAGE_SELF, &usage) != 0) return 8;
    printf("threads=%zu operations=%zu elapsed_ns=%llu ops_per_second=%.2f "
           "latency_ns=%.2f peak_rss_kib=%ld live_requested_bytes=%llu "
           "live_rss_delta_bytes=%llu fragmentation_ratio=%.4f\n",
           count, operations, (unsigned long long)elapsed,
           (double)operations * 1000000000.0 / (double)elapsed,
           (double)elapsed / (double)operations, usage.ru_maxrss,
           (unsigned long long)requested, (unsigned long long)live_delta,
           requested == 0 ? 0.0 : (double)live_delta / (double)requested);
    pthread_barrier_destroy(&ready_barrier);
    pthread_barrier_destroy(&start_barrier);
    return 0;
}

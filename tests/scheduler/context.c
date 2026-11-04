/* Each worker spends the test interval in userspace, with no syscall in the
 * sentinel loop. Completion remains observable through the detached console. */
#define _GNU_SOURCE
#include <pthread.h>
#include <cpuid.h>
#include <time.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/syscall.h>
#include <unistd.h>
#ifndef WORKERS
#define WORKERS 1
#endif
#ifndef SEED
#define SEED 0x1357
#endif
#ifndef CONTEXT_ITERATIONS
#define CONTEXT_ITERATIONS 100000000
#endif
extern int check_context(uint64_t iterations, uint64_t seed);
static __thread uint64_t tls_sentinel;
static unsigned cpu_mask;
static unsigned failed;
static unsigned hardware_cpu(void) {
    unsigned a, b, c, d;
    __cpuid(1, a, b, c, d);
    return b >> 24;
}
static void *worker(void *argument) {
    unsigned id = (unsigned)(uintptr_t)argument;
    tls_sentinel = SEED + id * 0x100;
    unsigned cpu = hardware_cpu();
    if (cpu < 4) __atomic_fetch_or(&cpu_mask, 1u << cpu, __ATOMIC_RELAXED);
    int result;
    do {
        result = check_context(CONTEXT_ITERATIONS, tls_sentinel);
#ifndef CONTEXT_CONTINUOUS
        break;
#endif
    } while (!result);
    if (result || tls_sentinel != SEED + id * 0x100) {
        __atomic_store_n(&failed, result ? (unsigned)result : 99, __ATOMIC_RELAXED);
    }
    cpu = hardware_cpu();
    if (cpu < 4) __atomic_fetch_or(&cpu_mask, 1u << cpu, __ATOMIC_RELAXED);
    return 0;
}
int main(void) {
    struct timespec begin, end;
    /* Ginux's monotonic epoch is per domain. Realtime uses the shared boot
     * epoch, so these intervals can be compared across detached domains. */
    if (clock_gettime(CLOCK_REALTIME, &begin)) return 4;
    pthread_t threads[WORKERS];
    for (unsigned i = 1; i < WORKERS; ++i) {
        int err = pthread_create(&threads[i], 0, worker, (void *)(uintptr_t)i);
        if (err) { printf("S1_CONTEXT_FAIL pthread_create=%d\n", err); fflush(stdout); return 2; }
    }
    worker(0);
    for (unsigned i = 1; i < WORKERS; ++i) pthread_join(threads[i], 0);
    if (clock_gettime(CLOCK_REALTIME, &end)) return 4;
    long long elapsed_us = (end.tv_sec - begin.tv_sec) * 1000000LL + (end.tv_nsec - begin.tv_nsec) / 1000;
    long long start_us = begin.tv_sec * 1000000LL + begin.tv_nsec / 1000;
    long long end_us = end.tv_sec * 1000000LL + end.tv_nsec / 1000;
    printf("S1_CONTEXT_%s workers=%u cpu_mask=%u error=%u seed=%u elapsed_us=%lld realtime_start_us=%lld realtime_end_us=%lld\n",
           failed ? "FAIL" : "PASS", WORKERS, cpu_mask, failed, SEED, elapsed_us, start_us, end_us);
    fflush(stdout);
    /* This ordinary console wait marks init ready and allows attaching to
     * inspect results after the no-input competition interval has ended. */
    char byte;
    if (read(0, &byte, 1) < 0) return 3;
    return failed ? 1 : 0;
}

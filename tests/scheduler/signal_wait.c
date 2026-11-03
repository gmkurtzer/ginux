#define _GNU_SOURCE
#include <cpuid.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <time.h>
#include <unistd.h>
#ifndef SEED
#define SEED 4951
#endif
extern int check_context(uint64_t, uint64_t);
static volatile sig_atomic_t hits, failure;
static void handler(int signal) {
    (void)signal;
    failure = check_context(20000000, SEED + 0x400);
    ++hits;
}
int main(void) {
    struct sigaction action = { .sa_handler = handler };
    sigemptyset(&action.sa_mask);
    struct timespec begin, end;
    clock_gettime(CLOCK_MONOTONIC, &begin);
    if (sigaction(SIGUSR1, &action, 0) || raise(SIGUSR1)) return 2;
    if (hits != 1 || failure) return 3;
    for (unsigned i = 0; i < 5; ++i) {
        struct timespec delay = { .tv_nsec = 50000000 };
        if (nanosleep(&delay, 0)) return 4;
    }
    clock_gettime(CLOCK_MONOTONIC, &end);
    long long elapsed_us = (end.tv_sec - begin.tv_sec) * 1000000LL + (end.tv_nsec - begin.tv_nsec) / 1000;
    unsigned a, b, c, d;
    __cpuid(1, a, b, c, d);
    unsigned cpu = b >> 24;
    printf("S1_CONTEXT_%s workers=1 cpu_mask=%u error=0 seed=%u elapsed_us=%lld signals=%d waits=5\n",
           elapsed_us >= 250000 ? "PASS" : "FAIL", cpu < 4 ? 1u << cpu : 0, SEED, elapsed_us, hits);
    fflush(stdout);
    char byte;
    if (read(0, &byte, 1) < 0) return 5;
    return 0;
}

#define _GNU_SOURCE
#include <errno.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

enum { THREADS = 8, ITERATIONS = 2000 };

struct release_batch {
    void *pointers[256];
};

static void *worker(void *argument) {
    uintptr_t id = (uintptr_t)argument;
    for (size_t i = 0; i < ITERATIONS; ++i) {
        size_t size = 1 + ((i * 131 + id * 17) % 8192);
        unsigned char *memory = malloc(size);
        if (memory == NULL) return (void *)1;
        memset(memory, (int)(id + i), size);
        size_t grown = size + 257;
        unsigned char *replacement = realloc(memory, grown);
        if (replacement == NULL) return (void *)2;
        free(replacement);
    }
    return NULL;
}

static void *release_worker(void *argument) {
    struct release_batch *batch = argument;
    for (size_t i = 0; i < 256; ++i) free(batch->pointers[i]);
    return NULL;
}

static int expect_double_free_abort(void) {
    pid_t child = fork();
    if (child < 0) return 1;
    if (child == 0) {
        void *memory = malloc(32);
        void (*volatile release)(void *) = free;
        if (memory == NULL) _exit(2);
        release(memory);
        release(memory);
        _exit(3);
    }
    int status = 0;
    if (waitpid(child, &status, 0) != child) return 4;
    return WIFEXITED(status) && WEXITSTATUS(status) == 134 ? 0 : 5;
}

static int expect_interior_free_abort(void) {
    pid_t child = fork();
    if (child < 0) return 1;
    if (child == 0) {
        unsigned char *memory = malloc(64);
        void (*volatile release)(void *) = free;
        volatile uintptr_t offset = 1;
        if (memory == NULL) _exit(2);
        release(memory + offset);
        _exit(3);
    }
    int status = 0;
    if (waitpid(child, &status, 0) != child) return 4;
    return WIFEXITED(status) && WEXITSTATUS(status) == 134 ? 0 : 5;
}

int main(void) {
    unsigned char *zero = calloc(64, 4);
    if (zero == NULL) return 10;
    for (size_t i = 0; i < 256; ++i) if (zero[i] != 0) return 11;
    free(zero);

    void *aligned = NULL;
    if (posix_memalign(&aligned, 4096, 8192) != 0 || aligned == NULL) return 12;
    if ((uintptr_t)aligned % 4096 != 0) return 13;
    memset(aligned, 0x5a, 8192);
    free(aligned);

    volatile size_t overflowing_count = (size_t)-1;
    void *(*volatile allocate_zeroed)(size_t, size_t) = calloc;
    errno = 0;
    if (allocate_zeroed(overflowing_count, 2) != NULL || errno != ENOMEM)
        return 14;

    pthread_t threads[THREADS];
    for (uintptr_t i = 0; i < THREADS; ++i)
        if (pthread_create(&threads[i], NULL, worker, (void *)i) != 0) return 20;
    for (size_t i = 0; i < THREADS; ++i) {
        void *result = NULL;
        if (pthread_join(threads[i], &result) != 0 || result != NULL) return 21;
    }

    struct release_batch batch;
    for (size_t i = 0; i < 256; ++i) {
        batch.pointers[i] = malloc(16 + i);
        if (batch.pointers[i] == NULL) return 22;
    }
    pthread_t releaser;
    if (pthread_create(&releaser, NULL, release_worker, &batch) != 0) return 23;
    if (pthread_join(releaser, NULL) != 0) return 24;

    if (expect_double_free_abort() != 0) return 30;
    if (expect_interior_free_abort() != 0) return 31;
    puts("GINUX_ALLOCATOR_CONFORMANCE: PASS");
    return 0;
}

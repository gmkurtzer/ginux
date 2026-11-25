#define _GNU_SOURCE
#include <errno.h>
#include <dlfcn.h>
#include <malloc.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

enum { THREADS = 8, ITERATIONS = 2000 };

enum { MALLOCX_ZERO_FLAG = 0x40 };

static int mallocx_align_flag(unsigned alignment) {
    int lg = 0;
    while ((1U << lg) < alignment) ++lg;
    return lg;
}

struct release_batch {
    void *pointers[256];
};

static int stats_callback_seen;

static void stats_callback(void *opaque, const char *message) {
    (void)opaque;
    if (message != NULL && strstr(message, "ginux-allocator") != NULL)
        stats_callback_seen = 1;
}

static int exercise_extended_abi(void) {
    void *(*mallocx_fn)(size_t, int) = dlsym(RTLD_DEFAULT, "mallocx");
    void *(*rallocx_fn)(void *, size_t, int) = dlsym(RTLD_DEFAULT, "rallocx");
    size_t (*xallocx_fn)(void *, size_t, size_t, int) = dlsym(RTLD_DEFAULT, "xallocx");
    size_t (*sallocx_fn)(void *, int) = dlsym(RTLD_DEFAULT, "sallocx");
    void (*dallocx_fn)(void *, int) = dlsym(RTLD_DEFAULT, "dallocx");
    void (*sdallocx_fn)(void *, size_t, int) = dlsym(RTLD_DEFAULT, "sdallocx");
    size_t (*nallocx_fn)(size_t, int) = dlsym(RTLD_DEFAULT, "nallocx");
    int (*mallctl_fn)(const char *, void *, size_t *, void *, size_t) =
        dlsym(RTLD_DEFAULT, "mallctl");
    int (*mallctlnametomib_fn)(const char *, size_t *, size_t *) =
        dlsym(RTLD_DEFAULT, "mallctlnametomib");
    int (*mallctlbymib_fn)(const size_t *, size_t, void *, size_t *, void *, size_t) =
        dlsym(RTLD_DEFAULT, "mallctlbymib");
    void (*stats_fn)(void (*)(void *, const char *), void *, const char *) =
        dlsym(RTLD_DEFAULT, "malloc_stats_print");
    if (mallocx_fn == NULL || rallocx_fn == NULL || xallocx_fn == NULL ||
        sallocx_fn == NULL || dallocx_fn == NULL || sdallocx_fn == NULL ||
        nallocx_fn == NULL || mallctl_fn == NULL || mallctlnametomib_fn == NULL ||
        mallctlbymib_fn == NULL || stats_fn == NULL)
        return 1;
    static const size_t sizes[] = {
        1, 15, 16, 17, 31, 32, 33, 63, 64, 65, 4096, 65536, 65537,
    };
    static const unsigned alignments[] = { 0, 8, 64, 4096 };
    for (size_t i = 0; i < sizeof(sizes) / sizeof(sizes[0]); ++i) {
        for (size_t j = 0; j < sizeof(alignments) / sizeof(alignments[0]); ++j) {
            int flags = alignments[j] == 0 ? 0 : mallocx_align_flag(alignments[j]);
            if ((i + j) % 2 != 0) flags |= MALLOCX_ZERO_FLAG;
            size_t predicted = nallocx_fn(sizes[i], flags);
            void *allocation = mallocx_fn(sizes[i], flags);
            if (allocation == NULL || predicted == 0 ||
                predicted != sallocx_fn(allocation, flags))
                return 2;
            if (alignments[j] != 0 &&
                (uintptr_t)allocation % alignments[j] != 0)
                return 3;
            if ((flags & MALLOCX_ZERO_FLAG) != 0) {
                const unsigned char *bytes = allocation;
                for (size_t k = 0; k < sizes[i]; ++k)
                    if (bytes[k] != 0) return 4;
            }
            dallocx_fn(allocation, flags);
        }
    }

    int initial_flags = mallocx_align_flag(64);
    void *first = mallocx_fn(33, initial_flags);
    if (first == NULL) return 5;
    memset(first, 0x5a, 33);
    int grown_flags = mallocx_align_flag(4096) | MALLOCX_ZERO_FLAG;
    first = rallocx_fn(first, 4097, grown_flags);
    if (first == NULL || (uintptr_t)first % 4096 != 0 ||
        sallocx_fn(first, grown_flags) != nallocx_fn(4097, grown_flags))
        return 6;
    const unsigned char *grown = first;
    for (size_t i = 0; i < 33; ++i) if (grown[i] != 0x5a) return 7;
    for (size_t i = 33; i < 4097; ++i) if (grown[i] != 0) return 8;
    if (xallocx_fn(first, 32, 0, 0) < 32) return 3;
    dallocx_fn(first, 0);
    errno = 0;
    if (mallocx_fn(32, 0x80) != NULL || errno != EINVAL ||
        nallocx_fn(32, 0x80) != 0)
        return 9;
    void *second = mallocx_fn(64, 0);
    if (second == NULL) return 10;
    sdallocx_fn(second, 64, 0);
    size_t mib = 0, mib_length = 1;
    if (mallctl_fn("unsupported", NULL, NULL, NULL, 0) != ENOENT ||
        mallctlnametomib_fn("unsupported", &mib, &mib_length) != ENOENT ||
        mallctlbymib_fn(&mib, mib_length, NULL, NULL, NULL, 0) != ENOENT)
        return 11;
    stats_fn(stats_callback, NULL, NULL);
    return stats_callback_seen ? 0 : 12;
}

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
    void (*cfree_fn)(void *) = dlsym(RTLD_DEFAULT, "cfree");
    if (cfree_fn == NULL) return 7;
    void *ordinary = malloc(17);
    if (ordinary == NULL || malloc_usable_size(ordinary) < 17) return 8;
    ordinary = reallocarray(ordinary, 4, 32);
    if (ordinary == NULL) return 9;
    cfree_fn(ordinary);

    unsigned char *zero = calloc(64, 4);
    if (zero == NULL) return 10;
    for (size_t i = 0; i < 256; ++i) if (zero[i] != 0) return 11;
    free(zero);

    void *aligned = NULL;
    if (posix_memalign(&aligned, 4096, 8192) != 0 || aligned == NULL) return 12;
    if ((uintptr_t)aligned % 4096 != 0) return 13;
    memset(aligned, 0x5a, 8192);
    free(aligned);

    aligned = aligned_alloc(64, 128);
    if (aligned == NULL || (uintptr_t)aligned % 64 != 0) return 15;
    free(aligned);
    aligned = memalign(128, 129);
    if (aligned == NULL || (uintptr_t)aligned % 128 != 0) return 16;
    free(aligned);
    aligned = valloc(33);
    if (aligned == NULL || (uintptr_t)aligned % (uintptr_t)sysconf(_SC_PAGESIZE) != 0)
        return 17;
    free(aligned);
    aligned = pvalloc(33);
    if (aligned == NULL || (uintptr_t)aligned % (uintptr_t)sysconf(_SC_PAGESIZE) != 0)
        return 18;
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
    if (exercise_extended_abi() != 0) return 32;
    puts("GINUX_ALLOCATOR_CONFORMANCE: PASS");
    return 0;
}

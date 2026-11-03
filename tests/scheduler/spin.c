/* Bounded by the QEMU harness timeout. Cooperative is a positive control. */
#include <unistd.h>
#include <sys/syscall.h>
int main(void) {
    static const char ready[] = "S1_SPIN_ENTERED\n";
    if (write(1, ready, sizeof ready - 1) != sizeof ready - 1) return 2;
    for (;;) {
#ifdef COOPERATIVE
        syscall(SYS_sched_yield);
#else
        __asm__ volatile("pause" ::: "memory");
#endif
    }
}

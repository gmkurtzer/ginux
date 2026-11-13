/* Freestanding x86-64 workload installed as /usr/bin/bash for `ginux native`
 * supervision checks. `native` always runs bash with fixed arguments, so the
 * scenario is read from /scenario in the instance root. */
typedef unsigned long word;
#ifdef CLONE_SNAPSHOT_COLLISION
__attribute__((used, section(".ginux_clone_collision")))
static const char occupied_snapshot_page[4096] = {0x5a};
#endif

static long call3(long number, word a, word b, word c)
{
    long result;
    __asm__ volatile("syscall" : "=a"(result)
                     : "a"(number), "D"(a), "S"(b), "d"(c)
                     : "rcx", "r11", "memory");
    return result;
}

static long call4(long number, word a, word b, word c, word d)
{
    long result;
    register word r10 __asm__("r10") = d;
    __asm__ volatile("syscall" : "=a"(result)
                     : "a"(number), "D"(a), "S"(b), "d"(c), "r"(r10)
                     : "rcx", "r11", "memory");
    return result;
}

static long call5(long number, word a, word b, word c, word d, word e)
{
    long result;
    register word r10 __asm__("r10") = d;
    register word r8 __asm__("r8") = e;
    __asm__ volatile("syscall" : "=a"(result)
                     : "a"(number), "D"(a), "S"(b), "d"(c), "r"(r10), "r"(r8)
                     : "rcx", "r11", "memory");
    return result;
}

static long call6(long number, word a, word b, word c, word d, word e, word f)
{
    long result;
    register word r10 __asm__("r10") = d;
    register word r8 __asm__("r8") = e;
    register word r9 __asm__("r9") = f;
    __asm__ volatile("syscall" : "=a"(result)
                     : "a"(number), "D"(a), "S"(b), "d"(c), "r"(r10), "r"(r8), "r"(r9)
                     : "rcx", "r11", "memory");
    return result;
}

static void print(const char *value)
{
    word count = 0;
    while (value[count]) ++count;
    call3(1, 1, (word)value, count);
}

/* One write per line: supervisor audit lines share stdout. */
static void print_result(const char *label, long value)
{
    char line[96];
    char digits[24];
    word length = 0, at = sizeof digits;
    word magnitude = value < 0 ? -(word)value : (word)value;
    do digits[--at] = '0' + magnitude % 10; while (magnitude /= 10);
    if (value < 0) digits[--at] = '-';
    while (*label && length < sizeof line - 26) line[length++] = *label++;
    while (at < sizeof digits) line[length++] = digits[at++];
    line[length++] = '\n';
    call3(1, 1, (word)line, length);
}

/* The supervisor answers TIOCGPGRP itself on a descriptor it tracks as the
 * console; any other descriptor gets the host's answer (ENOTTY or EBADF). */
static long tracked_as_console(long fd)
{
    int group = 0;
    return call3(16, fd, 0x540f /* TIOCGPGRP */, (word)&group) == 0;
}

static volatile long yield_signals;

static void yield_signal_handler(int signal)
{
    (void)signal;
    ++yield_signals;
}

static int sigchld_pipe = -1;
static volatile int sigchld_seen;

static void sigchld_handler(int signal)
{
    (void)signal;
    sigchld_seen = 1;
    call3(1, sigchld_pipe, (word)"c", 1);
}

static volatile int restart_signals;

static void restart_handler(int signal)
{
    (void)signal;
    ++restart_signals;
}

void sigchld_restorer(void);
__asm__(".global sigchld_restorer\n"
        "sigchld_restorer:\n"
        "mov $15, %rax\n"
        "syscall\n");

long native_thread_trap(word flags, word stack);
__asm__(".global native_thread_trap\n"
        "native_thread_trap:\n"
        "xor %edx, %edx\n"
        "xor %r10d, %r10d\n"
        "xor %r8d, %r8d\n"
        "mov $56, %eax\n"
        "syscall\n"
        "test %rax, %rax\n"
        "jnz 1f\n"
        "int3\n"
        "ud2\n"
        "1: ret\n");

long native_clone_spawn(void *args, word size);
__asm__(".global native_clone_spawn\n"
        "native_clone_spawn:\n"
        "mov $435, %rax\n"
        "syscall\n"
        "test %rax, %rax\n"
        "jnz 1f\n"
        "mov $7, %edi\n"
        "mov $231, %eax\n"
        "syscall\n"
        "ud2\n"
        "1: ret\n");

/* A CLONE_THREAD child that stores the result of syscall `number` with
 * arguments a and b in *result and exits. */
long native_thread_call(word flags, word stack, word number, word a, word b,
                        volatile long *result);
__asm__(".global native_thread_call\n"
        "native_thread_call:\n"
        "push %r12\n"
        "push %r13\n"
        "push %r14\n"
        "push %r15\n"
        "mov %rdx, %r12\n"
        "mov %rcx, %r13\n"
        "mov %r8, %r14\n"
        "mov %r9, %r15\n"
        "xor %edx, %edx\n"
        "xor %r10d, %r10d\n"
        "xor %r8d, %r8d\n"
        "mov $56, %eax\n"
        "syscall\n"
        "test %rax, %rax\n"
        "jnz 1f\n"
        "mov %r12, %rax\n"
        "mov %r13, %rdi\n"
        "mov %r14, %rsi\n"
        "xor %edx, %edx\n"
        "syscall\n"
        "mov %rax, (%r15)\n"
        "mov $60, %eax\n"
        "xor %edi, %edi\n"
        "syscall\n"
        "ud2\n"
        "1: pop %r15\n"
        "pop %r14\n"
        "pop %r13\n"
        "pop %r12\n"
        "ret\n");

long native_fixture_main(const word *stack)
{
    char scenario[16] = {0};
    long fd = call3(2, (word)"/scenario", 0, 0);
    if (fd < 0 || call3(0, fd, (word)scenario, sizeof scenario - 1) <= 0) return 80;
    switch (scenario[0]) {
    case 'f': {
        /* fork: the native loop has no child supervision, so it must be denied. */
        long pid = call3(57, 0, 0, 0);
        if (pid == 0) {
            print("GINUX_FIXTURE: unsupervised child ran\n");
            return 0;
        }
        print_result("GINUX_FIXTURE: fork=", pid);
        return 0;
    }
    case 'g': {
        /* owner: fcntl cannot set up signal-driven I/O; descriptor and
         * status flags still work on the test's stdout pipe. */
        struct { int type, pid; } owner = {1 /* F_OWNER_PID */, 0};
        long flags = call3(72, 1, 3, 0);
        print_result("GINUX_FIXTURE: setown=", call3(72, 1, 8, 0));
        print_result("GINUX_FIXTURE: setown-ex=", call3(72, 1, 15, (word)&owner));
        print_result("GINUX_FIXTURE: setsig=", call3(72, 1, 10, 15));
        print_result("GINUX_FIXTURE: async=", call3(72, 1, 4, flags | 020000));
        print_result("GINUX_FIXTURE: nonblock=", call3(72, 1, 4, flags | 04000));
        call3(72, 1, 4, flags);
        return 0;
    }
    case 'q': {
        print_result("GINUX_FIXTURE: fioasync=", call3(16, -1, 0x5452, 0));
        print_result("GINUX_FIXTURE: fiosetown=", call3(16, -1, 0x8901, 0));
        print_result("GINUX_FIXTURE: siocspgrp=", call3(16, -1, 0x8902, 0));
        print_result("GINUX_FIXTURE: high-fioasync=", call3(16, -1, 0xfeed000000005452UL, 0));
        print_result("GINUX_FIXTURE: tcgets=", call3(16, -1, 0x5401, 0));
        return 0;
    }
    case 't': {
        /* A plain SIGTRAP stop between syscalls; uname stays denied before and
         * after it unless supervision has lost track of entry and exit stops. */
        char buffer[6 * 65];
        print_result("GINUX_FIXTURE: uname-before=", call3(63, (word)buffer, 0, 0));
        __asm__ volatile("int3");
        print_result("GINUX_FIXTURE: uname-after=", call3(63, (word)buffer, 0, 0));
        return 0;
    }
    case 'T': {
        /* A thread takes a plain SIGTRAP stop while the leader spins without
         * syscalls, so supervision fails the instance with a traced thread
         * still alive. Native supervision denies nanosleep, and a write or
         * exit by the leader would race the thread's stop. */
        word stack = call6(9, 0, 16384, 3, 0x22, -1, 0);
        const word thread = 0x50f00; /* VM|FS|FILES|SIGHAND|THREAD|SYSVSEM */
        long created = native_thread_trap(thread, stack + 16384);
        if (created > 0)
            for (volatile long spin = 0; spin < 4000000000L; ++spin) {}
        print_result("GINUX_FIXTURE: thread=", created);
        return 0;
    }
    case 'i': {
        /* int 0x80 eax=2 is i386 fork; it must not be read as x86-64 open.
         * rsi = 0 makes it pass the x86-64 open flag check. */
        long pid;
        __asm__ volatile("int $0x80" : "=a"(pid) : "a"(2L), "b"(0L), "c"(0L), "d"(0L), "S"(0L)
                         : "memory");
        if (pid == 0) {
            print("GINUX_FIXTURE: unsupervised child ran\n");
            return 0;
        }
        print_result("GINUX_FIXTURE: int80-fork=", pid);
        return 0;
    }
    case 'v': {
        /* Thread identity: set_tid_address returns the caller's tid, which
         * for a single-threaded task equals getpid, and FUTEX_LOCK_PI on a
         * free word stores the caller's tid in it, also with flag bits above
         * the command (1024). Neither may expose the host tid next to the
         * virtual pid. */
        static int clear_tid;
        static int pi_word;
        static int pi_flags_word;
        print_result("GINUX_FIXTURE: set_tid_address=", call3(218, (word)&clear_tid, 0, 0));
        print_result("GINUX_FIXTURE: getpid=", call3(39, 0, 0, 0));
        print_result("GINUX_FIXTURE: gettid=", call3(186, 0, 0, 0));
        print_result("GINUX_FIXTURE: futex_lock_pi=", call4(202, (word)&pi_word, 6 | 128, 0, 0));
        print_result("GINUX_FIXTURE: pi_word=", pi_word);
        print_result("GINUX_FIXTURE: futex_lock_pi_flags=",
                     call4(202, (word)&pi_flags_word, 6 | 128 | 1024, 0, 0));
        print_result("GINUX_FIXTURE: pi_flags_word=", pi_flags_word);
        return 0;
    }
    case 'Y': {
        /* scripts/test-native-yield-broker.py releases each sched_yield with
         * a byte on stdin and interrupts the first two while the paranoid
         * broker holds their notifications. Without SA_RESTART an
         * interrupted yield returns -EINTR. */
        struct { word handler, flags, restorer, mask; } action = {
            (word)yield_signal_handler, 0x04000000 /* SA_RESTORER */, (word)sigchld_restorer, 0};
        static const char *const labels[] = {"GINUX_FIXTURE: yield-before-receive=",
                                             "GINUX_FIXTURE: yield-before-reply=",
                                             "GINUX_FIXTURE: yield-after=", 0};
        char go;
        if (call4(13, 10, (word)&action, 0, 8) != 0) return 82;
        for (const char *const *label = labels; *label; ++label) {
            if (call3(0, 0, (word)&go, 1) != 1) return 83;
            print_result(*label, call3(24, 0, 0, 0));
        }
        print_result("GINUX_FIXTURE: yield-signals=", yield_signals);
        return 0;
    }
    case 'y': {
        unsigned char info[112];
        for (word i = 0; i < sizeof info; ++i) info[i] = 0xaa;
        long result = call3(99, (word)info, 0, 0);
        word private = 1;
        for (word i = 0; i < sizeof info; ++i) {
            if ((i < 32 || i >= 40) && (i < 104 || i >= 108))
                private &= info[i] == 0;
        }
        print_result("GINUX_FIXTURE: sysinfo=", result);
        print_result("GINUX_FIXTURE: sysinfo-private=", private);
        print_result("GINUX_FIXTURE: totalram=", *(unsigned long *)(info + 32));
        print_result("GINUX_FIXTURE: mem-unit=", *(unsigned int *)(info + 104));
        print_result("GINUX_FIXTURE: sysinfo-null=", call3(99, 0, 0, 0));
        return 0;
    }
    case 'o': {
        static char input[1024 * 1024 + 1];
        long input_fd = call3(2, (word)"/large", 0, 0);
        if (input_fd < 0) return 82;
        print_result("GINUX_FIXTURE: read=", call3(0, input_fd, (word)input, sizeof input));
        print_result("GINUX_FIXTURE: pread=", call4(17, input_fd, (word)input, sizeof input, 0));
        return 0;
    }
    case 'r': {
        static char random[512];
        print_result("GINUX_FIXTURE: getrandom=", call3(318, (word)random, sizeof random, 0));
        return 0;
    }
    case 'd': {
        /* descriptors: the launcher passed a host directory as 239, 242 and
         * 300; only the protected null (240) and console (241) stay open. */
        print_result("GINUX_FIXTURE: fd-239=", call3(72, 239, 1, 0));
        print_result("GINUX_FIXTURE: fd-240=", call3(72, 240, 1, 0));
        print_result("GINUX_FIXTURE: fd-241=", call3(72, 241, 1, 0));
        print_result("GINUX_FIXTURE: fd-242=", call3(72, 242, 1, 0));
        print_result("GINUX_FIXTURE: fd-300=", call3(72, 300, 1, 0));
        return 0;
    }
    case 's': {
        /* SIGCHLD reaches a handler (systemd profile, where fork is
         * supervised): the parent catches SIGCHLD with a handler that writes
         * to a pipe, as OpenRC, busybox and shells do, forks a child that
         * exits, then polls the pipe. systemd reads SIGCHLD from a signalfd
         * instead, so it never depends on this delivery. */
        struct kernel_sigaction {
            word handler, flags, restorer, mask;
        } action = {(word)sigchld_handler, 0x04000000 /* SA_RESTORER */,
                    (word)sigchld_restorer, 0};
        struct pollfd_word {
            int fd;
            short events, revents;
        } poll_fd;
        int fds[2];
        int start_fds[2];
        int userspace = scenario[7] == '-';
        int status = 0;
        long pid;
        if (call4(293, (word)fds, 0, 0, 0) < 0) return 82;
        if (userspace && call4(293, (word)start_fds, 0, 0, 0) < 0) return 83;
        sigchld_pipe = fds[1];
        print_result("GINUX_FIXTURE: sigaction=", call4(13, 17, (word)&action, 0, 8));
        pid = call3(57, 0, 0, 0);
        if (pid == 0) {
            char go;
            if (userspace && call3(0, start_fds[0], (word)&go, 1) != 1) return 84;
            return 7;
        }
        print_result("GINUX_FIXTURE: fork-ok=", pid > 0);
        if (userspace) {
            /* Let the child exit only after the parent is ready to stay in
             * userspace. No poll or other syscall can supply an exit stop
             * while the parent waits for the actual signal handler. */
            if (call3(1, start_fds[1], (word)"g", 1) != 1) return 85;
            while (!sigchld_seen) __asm__ volatile("pause");
            print_result("GINUX_FIXTURE: sigchld-userspace=", sigchld_seen);
        }
        poll_fd.fd = fds[0];
        poll_fd.events = 1; /* POLLIN */
        poll_fd.revents = 0;
        long ready;
        /* A handler run interrupts the poll (EINTR) or makes the pipe readable. */
        do ready = call3(7, (word)&poll_fd, 1, 2000);
        while (ready == -4);
        print_result("GINUX_FIXTURE: sigchld-poll=", ready);
        print_result("GINUX_FIXTURE: sigchld-handled=", sigchld_seen);
        print_result("GINUX_FIXTURE: wait-ok=", call4(61, -1, (word)&status, 0, 0) == pid);
        print_result("GINUX_FIXTURE: child-status=", (status >> 8) & 0xff);
        return 0;
    }
    case 'p': {
        /* waitid(P_PIDFD), as systemd 258+ waits for its children: the result
         * names the child by its virtual pid. A pidfd of a non-child is
         * ECHILD and a descriptor that is not a pidfd is EBADF. */
        struct { int signo, error, code, pad, pid, uid, status, rest[25]; } info = {0};
        long pid = call3(57, 0, 0, 0);
        if (pid == 0) return 7;
        long pidfd = call3(434, pid, 0, 0);
        long self = call3(434, call3(39, 0, 0, 0), 0, 0);
        print_result("GINUX_FIXTURE: pidfd-open-ok=", pidfd >= 0 && self >= 0);
        print_result("GINUX_FIXTURE: waitid-self=", call5(247, 3, self, (word)&info, 4, 0));
        print_result("GINUX_FIXTURE: waitid-not-pidfd=", call5(247, 3, 1, (word)&info, 4, 0));
        print_result("GINUX_FIXTURE: waitid-invalid=", call5(247, 3, -1, (word)&info, 4, 0));
        print_result("GINUX_FIXTURE: waitid-high=", call5(247, 3, (word)pidfd | 0xfeed000000000000UL, (word)&info, 0x01000004, 0));
        print_result("GINUX_FIXTURE: waitid=", call5(247, 3, pidfd, (word)&info, 4, 0));
        print_result("GINUX_FIXTURE: waited-child=", info.pid == pid);
        print_result("GINUX_FIXTURE: child-status=", info.status);
        return 0;
    }
    case 'b': {
        /* Early denials of virtual-pid lookups: each must finish its audit
         * record, or the next audited syscall waits on the audit lock. */
        struct { int signo, error, code, pad, pid, uid, status, rest[25]; } info = {0};
        print_result("GINUX_FIXTURE: pidfd-open-unknown=", call3(434, 0x7ffffff0, 0, 0));
        print_result("GINUX_FIXTURE: waitid-unknown-pid=", call5(247, 1, 0x7ffffff0, (word)&info, 4, 0));
        print_result("GINUX_FIXTURE: waitid-bad-type=", call5(247, 7, 1, (word)&info, 4, 0));
        print_result("GINUX_FIXTURE: getpid-after=", call3(39, 0, 0, 0) > 0);
        return 0;
    }
    case 'c': {
        /* clone3 CLONE_INTO_CGROUP, as glibc 2.41+ posix_spawn starts every
         * systemd 258+ service: the cgroup is a directory of the virtual tree. */
        struct {
            word flags, pidfd, child_tid, parent_tid, exit_signal, stack, stack_size, tls,
                set_tid, set_tid_size, cgroup;
        } args = {0};
        int status = 0;
        long cgroup = call3(2, (word)"/sys/fs/cgroup", 0x10000 /* O_DIRECTORY */, 0);
        args.flags = 0x200000000UL; /* CLONE_INTO_CGROUP */
        args.exit_signal = 17;
        args.cgroup = (word)cgroup;
        args.cgroup = (word)-1;
        print_result("GINUX_FIXTURE: clone3-invalid=", call3(435, (word)&args, sizeof args, 0));
        args.cgroup = 1;
        print_result("GINUX_FIXTURE: clone3-ordinary=", call3(435, (word)&args, sizeof args, 0));
        args.cgroup = (word)cgroup;
        long pid = call3(435, (word)&args, sizeof args, 0);
        if (pid == 0) return 7;
        print_result("GINUX_FIXTURE: cgroup-open-ok=", cgroup >= 0);
        print_result("GINUX_FIXTURE: clone3-ok=", pid > 0);
        print_result("GINUX_FIXTURE: wait-ok=", pid > 0 && call4(61, pid, (word)&status, 0, 0) == pid);
        print_result("GINUX_FIXTURE: child-status=", (status >> 8) & 0xff);
        return 0;
    }
    case 'C': {
        /* Configured-native process creation must apply the same managed
         * shape policy before either clone ABI can reach the host kernel. */
        word stack = call6(9, 0, 16384, 3, 0x22, -1, 0);
        const word unsafe = 0x00800000 | 0x00000100 | 17; /* UNTRACED|VM|SIGCHLD */
        print_result("GINUX_FIXTURE: clone-untraced=",
                     call6(56, unsafe, stack + 16384, 0, 0, 0, 0));
        struct {
            word flags, pidfd, child_tid, parent_tid, exit_signal, stack, stack_size, tls,
                set_tid, set_tid_size, cgroup;
        } args = {0};
        args.flags = 0x00800000 | 0x00000100; /* UNTRACED|VM */
        args.exit_signal = 17;
        args.stack = stack;
        args.stack_size = 16384;
        print_result("GINUX_FIXTURE: clone3-untraced=",
                     call3(435, (word)&args, sizeof args, 0));
        return 0;
    }
    case 'a': {
        struct clone_args { word flags, pidfd, child_tid, parent_tid, exit_signal,
            stack, stack_size, tls, set_tid, set_tid_size, cgroup; };
        long cgroup = call3(2, (word)"/sys/fs/cgroup", 0x10000, 0);
        struct clone_args *args = (void *)call6(9, 0, 4096, 3, 0x22, -1, 0);
        if ((word)args > (word)-4096) return 83;
        args->flags = 0x300000000UL; /* INTO_CGROUP and CLEAR_SIGHAND */
        args->exit_signal = 17; args->cgroup = cgroup;
        struct { word handler, flags, restorer, mask; } action = {0x1234, 0, 0, 0}, old = {0};
        if (call4(13, 10, (word)&action, 0, 8) != 0) return 84;
        if (call3(10, (word)args, 4096, 1) != 0) return 85;
        long pid = call3(435, (word)args, sizeof *args, 0);
        if (pid == 0) {
            if (call4(13, 10, 0, (word)&old, 8) != 0 || old.handler != 0) return 86;
            return args->flags == 0x300000000UL ? 7 : 87;
        }
        int status = 0;
        print_result("GINUX_FIXTURE: readonly-clone=", pid > 0 && call4(61, pid, (word)&status, 0, 0) == pid && status == 1792);
        print_result("GINUX_FIXTURE: readonly-unchanged=", args->flags == 0x300000000UL);
        pid = native_clone_spawn(args, 96); /* zero extension tail */
        print_result("GINUX_FIXTURE: clone-zero-tail=", pid > 0 && call4(61, pid, (word)&status, 0, 0) == pid && status == 1792);
        int child_pidfd = -1;
        call3(10, (word)args, 4096, 3);
        args->flags = 0x300001000UL; /* preserve the declared PIDFD output */
        args->pidfd = (word)&child_pidfd;
        call3(10, (word)args, 4096, 1);
        pid = native_clone_spawn(args, 88);
        print_result("GINUX_FIXTURE: clone-pidfd=", pid > 0 && call4(61, pid, (word)&status, 0, 0) == pid && status == 1792 && child_pidfd >= 0 && call3(72, child_pidfd, 1, 0) == 1);
        print_result("GINUX_FIXTURE: pidfd-input=", args->flags == 0x300001000UL && args->pidfd == (word)&child_pidfd);
        if (child_pidfd >= 0) call3(3, child_pidfd, 0, 0);
        call3(10, (word)args, 4096, 3);
        args->pidfd = 0;
        call3(10, (word)args, 4096, 1);
        word child_stack = call6(9, 0, 16384, 3, 0x22, -1, 0);
        call3(10, (word)args, 4096, 3);
        args->flags = 0x300004100UL; /* VM | VFORK | CLEAR_SIGHAND | INTO_CGROUP */
        args->stack = child_stack; args->stack_size = 16384;
        call3(10, (word)args, 4096, 1);
        pid = native_clone_spawn(args, 88);
        print_result("GINUX_FIXTURE: clone-vfork=", pid > 0 && call4(61, pid, (word)&status, 0, 0) == pid && status == 1792);
        print_result("GINUX_FIXTURE: vfork-unchanged=", args->flags == 0x300004100UL && args->stack == child_stack && args->stack_size == 16384);
        call3(10, (word)args, 4096, 3);
        args->flags = 0x300000000UL; args->stack = 0; args->stack_size = 0;
        call3(10, (word)args, 4096, 1);
        print_result("GINUX_FIXTURE: clone-short=", call3(435, (word)args, 63, 0));
        print_result("GINUX_FIXTURE: clone-large=", call3(435, (word)args, 4097, 0));
        print_result("GINUX_FIXTURE: clone-unreadable=", call3(435, 1, 88, 0));
        print_result("GINUX_FIXTURE: clone-short-cgroup=", call3(435, (word)args, 80, 0));
        call3(10, (word)args, 4096, 3);
        args->flags = 0x200010000UL; /* invalid THREAD without SIGHAND */
        call3(10, (word)args, 4096, 1);
        print_result("GINUX_FIXTURE: clone-kernel-error=", call3(435, (word)args, 88, 0));
        print_result("GINUX_FIXTURE: error-unchanged=", args->flags == 0x200010000UL);
        call3(10, (word)args, 4096, 3);
        ((char *)args)[88] = 1;
        print_result("GINUX_FIXTURE: clone-tail=", call3(435, (word)args, 89, 0));
        ((char *)args)[88] = 0;
        args->flags = 0; /* version 0 remains a real clone3 */
        pid = call3(435, (word)args, 64, 0);
        if (pid == 0) return 7;
        print_result("GINUX_FIXTURE: clone-version0=", pid > 0 && call4(61, pid, (word)&status, 0, 0) == pid && status == 1792);
        args->flags = 0x300000000UL;
        call3(10, (word)args, 4096, 1);
        const word reserved = 0x600000000000UL;
        print_result("GINUX_FIXTURE: snapshot-mmap=", call6(9, reserved, 4096, 3, 0x100022, -1, 0));
        print_result("GINUX_FIXTURE: snapshot-mprotect=", call3(10, reserved, 4096, 3));
        print_result("GINUX_FIXTURE: snapshot-munmap=", call3(11, reserved, 4096, 0));
        print_result("GINUX_FIXTURE: snapshot-madvise=", call3(28, reserved, 4096, 4));
        print_result("GINUX_FIXTURE: snapshot-mremap=", call6(25, (word)args, 4096, 4096, 3, reserved, 0));
        long repeated = 0;
        for (int round = 0; round < 160; ++round) {
            pid = call3(435, (word)args, 88, 0);
            if (pid == 0) return 7;
            if (pid <= 0 || call4(61, pid, (word)&status, 0, 0) != pid || status != 1792) break;
            ++repeated;
        }
        print_result("GINUX_FIXTURE: snapshot-repeated=", repeated);
        return 0;
    }
    case 'u': {
        struct { word flags, pidfd, child_tid, parent_tid, exit_signal, stack,
            stack_size, tls, set_tid, set_tid_size, cgroup; } args = {0};
        args.flags = 0x300000000UL; args.exit_signal = 17;
        args.cgroup = call3(2, (word)"/sys/fs/cgroup", 0x10000, 0);
        print_result("GINUX_FIXTURE: race-args=", (word)&args);
        long pid = call3(435, (word)&args, sizeof args, 0);
        if (pid == 0) return 7;
        int status = 0;
        print_result("GINUX_FIXTURE: race-child=", pid > 0 && call4(61, pid, (word)&status, 0, 0) == pid && status == 1792);
        print_result("GINUX_FIXTURE: race-caller-mutation=", args.flags == 0x8000000200000000UL);
        return 0;
    }
    case 'x': {
        /* scripts/test-native-clone-restart.py queues an ignored SIGUSR1
         * while the replayed clone3 runs; the kernel abandons that call with
         * ERESTARTNOINTR, which the caller must never observe. */
        struct { word flags, pidfd, child_tid, parent_tid, exit_signal, stack,
            stack_size, tls, set_tid, set_tid_size, cgroup; } args = {0};
        struct { word handler, flags, restorer, mask; } ignore = {1 /* SIG_IGN */, 0, 0, 0};
        args.exit_signal = 17;
        if (call4(13, 10, (word)&ignore, 0, 8) != 0) return 88;
        long pid = call3(435, (word)&args, sizeof args, 0);
        if (pid == 0) return 7;
        int status = 0;
        print_result("GINUX_FIXTURE: restart-clone3=", pid > 0 ? 1 : pid);
        print_result("GINUX_FIXTURE: restart-child=", pid > 0 && call4(61, pid, (word)&status, 0, 0) == pid && status == 1792);
        return 0;
    }
#ifdef CLONE_SNAPSHOT_COLLISION
    case 'k': {
        struct { word flags, pidfd, child_tid, parent_tid, exit_signal, stack,
            stack_size, tls, set_tid, set_tid_size, cgroup; } args = {0};
        args.flags = 0x200000000UL; args.exit_signal = 17;
        args.cgroup = call3(2, (word)"/sys/fs/cgroup", 0x10000, 0);
        print_result("GINUX_FIXTURE: snapshot-collision=", call3(435, (word)&args, sizeof args, 0));
        print_result("GINUX_FIXTURE: collision-input=", args.flags == 0x200000000UL);
        const volatile unsigned char *page = (void *)0x600000000000UL;
        int intact = page[0] == 0x5a;
        for (int i = 1; i < 4096; ++i) if (page[i] != 0) intact = 0;
        print_result("GINUX_FIXTURE: collision-mapping=", intact);
        return 0;
    }
#endif
    case 'h': {
        /* PIDFD_GET_INFO (Linux 6.13) would report the child's host pid and
         * credentials; the 6.12 ABI personality has no such ioctl. */
        word info[8] = {1 /* PIDFD_INFO_PID */};
        int status = 0;
        long pid = call3(57, 0, 0, 0);
        if (pid == 0) return 0;
        long pidfd = call3(434, pid, 0, 0);
        print_result("GINUX_FIXTURE: pidfd-open-ok=", pidfd >= 0);
        print_result("GINUX_FIXTURE: pidfd-info=", call3(16, pidfd, 0xc040ff0b, (word)info));
        print_result("GINUX_FIXTURE: pidfd-info-high=", call3(16, pidfd, 0xfeed0000c008ff0bUL, (word)info));
        print_result("GINUX_FIXTURE: pidfd-info-invalid=", call3(16, -1, 0xc080ff0b, 0));
        print_result("GINUX_FIXTURE: unrelated-ioctl=", call3(16, -1, 0x5401, 0));
        print_result("GINUX_FIXTURE: wait-ok=", call4(61, pid, (word)&status, 0, 0) == pid);
        return 0;
    }
    case 'm': {
        /* Children reaped by waitid release their process slots: more children
         * than the native process table holds, one at a time. A WNOWAIT peek
         * first must leave the child to be reaped. */
        struct { int signo, error, code, pad, pid, uid, status, rest[25]; } info = {0};
        long reaped = 0;
        long pid = call3(57, 0, 0, 0);
        if (pid == 0) return 0;
        print_result("GINUX_FIXTURE: waitid-peek=", call5(247, 1, pid, (word)&info, 0x01000004, 0));
        print_result("GINUX_FIXTURE: waitid-after-peek=", call5(247, 1, pid, (word)&info, 4, 0));
        for (int round = 0; round < 200; ++round) {
            pid = call3(57, 0, 0, 0);
            if (pid == 0) return 0;
            if (pid < 0 || call5(247, 1, pid, (word)&info, 4, 0) != 0) break;
            ++reaped;
        }
        print_result("GINUX_FIXTURE: waitid-reaped=", reaped);
        return 0;
    }
    case 'n': {
        int status = 0, fds[2];
        struct { int signo, error, code, pad, pid, uid, status, rest[25]; } info = {0};
        if (call3(293, (word)fds, 0, 0) < 0) return 83;
        long pid = call3(57, 0, 0, 0);
        if (pid == 0) { char go; call3(0, fds[0], (word)&go, 1); return 7; }
        print_result("GINUX_FIXTURE: null-nohang=", call5(247, 1, pid, 0, 5, 0));
        call3(1, fds[1], (word)"g", 1);
        word selected = pid;
        long result;
        register word r10 __asm__("r10") = 0x01000004;
        register word r8 __asm__("r8") = 0;
        __asm__ volatile("syscall" : "=a"(result), "+S"(selected)
                         : "a"(247L), "D"(1L), "d"(0L), "r"(r10), "r"(r8)
                         : "rcx", "r11", "memory");
        print_result("GINUX_FIXTURE: null-peek=", result);
        print_result("GINUX_FIXTURE: null-selector-preserved=", selected == (word)pid);
        print_result("GINUX_FIXTURE: null-after-peek=", call5(247, 1, pid, 0, 4, 0));
        print_result("GINUX_FIXTURE: null-rewait=", call4(61, pid, (word)&status, 0, 0));
        pid = call3(57, 0, 0, 0);
        if (pid == 0) return 7;
        print_result("GINUX_FIXTURE: null-invalid-options=", call5(247, 1, pid, 0, 0x84, 0));
        print_result("GINUX_FIXTURE: null-rusage-fault=", call5(247, 1, pid, 0, 4, 1));
        print_result("GINUX_FIXTURE: fault-rewait=", call4(61, pid, (word)&status, 0, 0));
        long reaped = 0;
        for (int round = 0; round < 800; ++round) {
            pid = call3(57, 0, 0, 0);
            if (pid == 0) return 7;
            long pidfd = round % 3 == 2 ? call3(434, pid, 0, 0) : -1;
            long which = round % 3 == 0 ? 1 : round % 3 == 1 ? 0 : 3;
            long selected = which == 1 ? pid : which == 3 ? pidfd : 0;
            if (round % 20 == 0 && call5(247, which, selected, (word)&info, 0x01000004, 0) != 0) return 84;
            word usage[18] = {0};
            if (pid < 0 || call5(247, which, selected, 0, 4, (word)usage) != 0) break;
            if (pidfd >= 0) call3(3, pidfd, 0, 0);
            ++reaped;
        }
        print_result("GINUX_FIXTURE: null-reaped=", reaped);
        return 0;
    }
    case 'j': {
        /* Translated pid selectors: kill and pidfd_open leave the caller's
         * virtual pid in RDI, as the syscall ABI preserves it, and a wait4
         * that an SA_RESTART handler interrupts selects the same child when
         * the kernel restarts it. */
        struct kernel_sigaction {
            word handler, flags, restorer, mask;
        } action = {(word)restart_handler, 0x14000000 /* SA_RESTORER | SA_RESTART */,
                    (word)sigchld_restorer, 0};
        int fds[2], status = 0;
        long self = call3(39, 0, 0, 0), result;
        word selected = self;
        __asm__ volatile("syscall" : "=a"(result), "+D"(selected)
                         : "a"(62L), "S"(0L)
                         : "rcx", "r11", "memory");
        print_result("GINUX_FIXTURE: kill-self=", result);
        print_result("GINUX_FIXTURE: kill-selector-preserved=", selected == (word)self);
        selected = self;
        __asm__ volatile("syscall" : "=a"(result), "+D"(selected)
                         : "a"(434L), "S"(0L)
                         : "rcx", "r11", "memory");
        print_result("GINUX_FIXTURE: pidfd-open-ok=", result >= 0);
        print_result("GINUX_FIXTURE: pidfd-selector-preserved=", selected == (word)self);
        if (call4(293, (word)fds, 0, 0, 0) < 0) return 82;
        if (call4(13, 10, (word)&action, 0, 8) != 0) return 83;
        long slow = call3(57, 0, 0, 0);
        if (slow == 0) {
            char go;
            call3(0, fds[0], (word)&go, 1);
            return 7;
        }
        long helper = call3(57, 0, 0, 0);
        if (helper == 0) {
            /* Interrupt the parent's wait repeatedly before the slow child
             * may exit. */
            for (int round = 0; round < 20; ++round) {
                call3(7, 0, 0, 10);
                call3(62, self, 10, 0);
            }
            call3(1, fds[1], (word)"g", 1);
            return 0;
        }
        long waited = call4(61, slow, (word)&status, 0, 0);
        print_result("GINUX_FIXTURE: restarted-wait=", waited == slow);
        print_result("GINUX_FIXTURE: child-status=", (status >> 8) & 0xff);
        print_result("GINUX_FIXTURE: signal-handled=", restart_signals > 0);
        print_result("GINUX_FIXTURE: helper-wait=", call4(61, helper, (word)&status, 0, 0) == helper);
        return 0;
    }
    case 'e': {
        /* A receiver can consume SCM_CREDENTIALS after waitid has reaped the
         * short-lived sender. The queued identity must stay virtual. */
        struct iovec { void *base; word length; };
        struct message {
            void *name; unsigned name_length, name_padding;
            struct iovec *iov; word iov_count;
            void *control; word control_length;
            unsigned flags, flags_padding;
        };
        struct credentials { int pid; unsigned uid, gid; };
        struct control_header { word length; int level, type; };
        int fds[2] = {-1, -1}, enabled = 1;
        char payload = 0;
        word control[8] = {0};
        struct iovec iov = {&payload, 1};
        struct message message = {0, 0, 0, &iov, 1, control, sizeof control, 0, 0};
        struct { int signo, error, code, pad, pid, uid, status, rest[25]; } info = {0};
        if (call4(53, 1, 2, 0, (word)fds) < 0
            || call5(54, fds[0], 1, 16, (word)&enabled, sizeof enabled) < 0) return 85;
        long pid = call3(57, 0, 0, 0);
        if (pid == 0) {
            call3(1, fds[1], (word)"x", 1);
            call3(231, 0, 0, 0);
            return 86;
        }
        long waited = call5(247, 1, pid, (word)&info, 4, 0);
        long received = call3(47, fds[0], (word)&message, 0);
        struct control_header *header = (void *)control;
        struct credentials *credentials = (void *)(header + 1);
        print_result("GINUX_FIXTURE: credential-wait=", waited);
        print_result("GINUX_FIXTURE: credential-recv=", received);
        print_result("GINUX_FIXTURE: credential-pid=",
                     header->length >= 28 && header->level == 1 && header->type == 2
                     && credentials->pid == pid);
        print_result("GINUX_FIXTURE: credential-uid=", credentials->uid == 0);
        print_result("GINUX_FIXTURE: credential-gid=", credentials->gid == 0);
        return 0;
    }
    case 'z': {
        word selected = 0xfeed000000000007UL;
        long result;
        register word r10 __asm__("r10") = 4;
        register word r8 __asm__("r8") = 0;
        __asm__ volatile("syscall" : "=a"(result), "+S"(selected)
                         : "a"(247L), "D"(1L), "d"(0L), "r"(r10), "r"(r8)
                         : "rcx", "r11", "memory");
        print_result("GINUX_FIXTURE: denied-waitid=", result);
        print_result("GINUX_FIXTURE: denied-selector-preserved=", selected == 0xfeed000000000007UL);
        return 0;
    }
    case 'l': {
        /* mkdir in the virtual cgroup tree creates the interface files a
         * service manager attaches through, and stays inside the instance
         * root even through a symlink in the tree. */
        long made = call3(83, (word)"/sys/fs/cgroup/ginux-test.slice", 0755, 0);
        long procs = call4(257, (word)-100, (word)"/sys/fs/cgroup/ginux-test.slice/cgroup.procs",
                           1 /* O_WRONLY */, 0);
        print_result("GINUX_FIXTURE: mkdir=", made);
        print_result("GINUX_FIXTURE: procs-open-ok=", procs >= 0);
        print_result("GINUX_FIXTURE: procs-write=", procs >= 0 ? call3(1, procs, (word)"0\n", 2) : procs);
        print_result("GINUX_FIXTURE: mkdir-again=",
                     call3(83, (word)"/sys/fs/cgroup/ginux-test.slice", 0755, 0));
        print_result("GINUX_FIXTURE: mkdirat=",
                     call4(258, -100, (word)"/sys/fs/cgroup/at.slice", 0755, 0));
        long dir = call3(2, (word)"/sys/fs/cgroup/ginux-test.slice", 0x10000, 0);
        print_result("GINUX_FIXTURE: mkdirat-relative=",
                     call4(258, dir, (word)"child.slice", 0755, 0));
        print_result("GINUX_FIXTURE: events-open-ok=",
                     call3(2, (word)"/sys/fs/cgroup/ginux-test.slice/cgroup.events", 0, 0) >= 0);
        print_result("GINUX_FIXTURE: kill-open-ok=",
                     call3(2, (word)"/sys/fs/cgroup/ginux-test.slice/cgroup.kill", 1, 0) >= 0);
        print_result("GINUX_FIXTURE: mkdir-via-symlink=",
                     call3(83, (word)"/sys/fs/cgroup/escape/ginux-cgroup-escape-check", 0755, 0));
        return 0;
    }
    case 'L': {
        /* Console descriptor tracking follows the host descriptor table.
         * The exec at the end runs this scenario again with "after-exec". */
        const char *expected = "after-exec", *argument = stack[0] > 1 ? (const char *)stack[2] : "";
        while (*expected && *argument == *expected) ++argument, ++expected;
        if (*expected == 0 && *argument == 0) {
            /* After the exec below: the scenario descriptor reused the
             * number of the close-on-exec open of the console and is a plain
             * file, the close-on-exec copies 30-32, 36-37 and 42 are gone, and
             * 33-35 and 38 are still the console. */
            print_result("GINUX_FIXTURE: exec-reused-console=", tracked_as_console(fd));
            print_result("GINUX_FIXTURE: exec-write-reused=", call3(1, fd, (word)"L\n", 2));
            print_result("GINUX_FIXTURE: exec-dropped-console=",
                         tracked_as_console(30) + tracked_as_console(31) + tracked_as_console(32)
                         + tracked_as_console(36) + tracked_as_console(37) + tracked_as_console(42));
            print_result("GINUX_FIXTURE: exec-kept-console=",
                         tracked_as_console(33) + tracked_as_console(34) + tracked_as_console(35)
                         + tracked_as_console(38));
            return 0;
        }
        struct { word flags, pidfd, child_tid, parent_tid, exit_signal, stack,
            stack_size, tls, set_tid, set_tid_size, cgroup; } args = {0};
        int status = 0;
        /* The protected console descriptor survives close, which is only
         * emulated. */
        call3(3, 241, 0, 0);
        print_result("GINUX_FIXTURE: protected-close-console=", tracked_as_console(241));
        long console = call3(2, (word)"/dev/console", 1 /* O_WRONLY */, 0);
        print_result("GINUX_FIXTURE: console-open-ok=", console >= 0);
        /* The kernel reads a descriptor argument as 32 bits. */
        long high = call3(32, (1UL << 32) | (word)console, 0, 0);
        print_result("GINUX_FIXTURE: high-bits-dup-console=", tracked_as_console(high));
        call3(3, high, 0, 0);
        /* A CLONE_FILES child shares the table: its duplicate is the
         * parent's console descriptor too. */
        long pid = call5(56, 0x400 /* CLONE_FILES */ | 17, 0, 0, 0, 0);
        if (pid == 0) call3(231, call3(32, console, 0, 0), 0, 0);
        long shared = pid > 0 && call4(61, pid, (word)&status, 0, 0) == pid ? (status >> 8) & 0xff : -1;
        print_result("GINUX_FIXTURE: shared-dup-console=", tracked_as_console(shared));
        /* A console descriptor it closes and reuses for a file is not. */
        long copy = call3(32, console, 0, 0);
        args.flags = 0x400; args.exit_signal = 17;
        pid = call3(435, (word)&args, sizeof args, 0);
        if (pid == 0) {
            call3(3, copy, 0, 0);
            call3(231, call3(2, (word)"/scenario", 0, 0) == copy, 0, 0);
        }
        print_result("GINUX_FIXTURE: shared-reuse-same-number=",
                     pid > 0 && call4(61, pid, (word)&status, 0, 0) == pid && status == 256);
        print_result("GINUX_FIXTURE: shared-reuse-console=", tracked_as_console(copy));
        /* Without CLONE_FILES the child closes only its own copy. */
        long kept = call3(32, console, 0, 0);
        pid = call3(57, 0, 0, 0);
        if (pid == 0) call3(231, call3(3, kept, 0, 0), 0, 0);
        if (pid > 0) call4(61, pid, (word)&status, 0, 0);
        print_result("GINUX_FIXTURE: private-close-console=", tracked_as_console(kept));
        /* A thread shares the table as well, and its open of the console is
         * the latest open for the other threads too. */
        volatile long threaded = -1000, opened = -1000;
        word thread_stack = call6(9, 0, 16384, 3, 0x22, -1, 0);
        const word thread = 0x50f00; /* VM|FS|FILES|SIGHAND|THREAD|SYSVSEM */
        if (native_thread_call(thread, thread_stack + 16384, 32, console, 0, &threaded) > 0)
            while (threaded == -1000) call3(24, 0, 0, 0);
        print_result("GINUX_FIXTURE: thread-dup-console=", tracked_as_console(threaded));
        if (native_thread_call(thread, thread_stack + 16384, 2, (word)"/dev/console", 1,
                               &opened) > 0)
            while (opened == -1000) call3(24, 0, 0, 0);
        print_result("GINUX_FIXTURE: write-thread-open=", call3(1, opened, (word)"L\n", 2));
        /* dup2 and dup3 of a file over a console copy replace it. */
        call3(72, console, 0, 39);
        call3(33, fd, 39, 0);
        call3(72, console, 0, 40);
        call3(292, fd, 40, 0x80000);
        print_result("GINUX_FIXTURE: overwritten-console=",
                     tracked_as_console(39) + tracked_as_console(40));
        /* close_range without flags closes. */
        call3(72, console, 0, 41);
        call3(436, 41, 41, 0);
        print_result("GINUX_FIXTURE: closed-range-console=", tracked_as_console(41));
        call3(3, console, 0, 0);
        call3(3, shared, 0, 0);
        call3(3, copy, 0, 0);
        call3(3, kept, 0, 0);
        call3(3, threaded, 0, 0);
        call3(3, opened, 0, 0);
        /* Exec closes the close-on-exec descriptors, however they got the
         * flag, and keeps the rest. The open itself must give the kernel
         * descriptor the flag and leave the caller's arguments in place. */
        word path = (word)"/dev/console", flags = 1 | 0x80000 /* O_CLOEXEC */, mode = 0;
        __asm__ volatile("syscall" : "=a"(console), "+D"(path), "+S"(flags), "+d"(mode)
                         : "a"(2L)
                         : "rcx", "r11", "memory");
        print_result("GINUX_FIXTURE: console-cloexec=", call3(72, console, 1 /* F_GETFD */, 0));
        print_result("GINUX_FIXTURE: open-arguments-kept=",
                     path == (word)"/dev/console" && flags == (1 | 0x80000) && mode == 0);
        call3(292, console, 30, 0x80000);
        call3(72, console, 1030 /* F_DUPFD_CLOEXEC */, 31);
        call3(72, call3(72, console, 0 /* F_DUPFD */, 32), 2 /* F_SETFD */, 1 /* FD_CLOEXEC */);
        call3(72, console, 0, 33);
        call3(33, console, 34, 0);
        call3(72, call3(72, console, 1030, 35), 2, 0);
        call3(16, call3(72, console, 0, 36), 0x5451 /* FIOCLEX */, 0);
        call3(72, console, 0, 37);
        call3(436, 37, 37, 4 /* CLOSE_RANGE_CLOEXEC */);
        print_result("GINUX_FIXTURE: cloexec-range-console=", tracked_as_console(37));
        call3(292, console, 38, 0x80000);
        call3(16, 38, 0x5450 /* FIONCLEX */, 0);
        call3(72, console, 1030, 42);
        call3(33, 42, 42, 0); /* dup2 onto itself keeps close-on-exec */
        word exec_argv[] = {(word)"/usr/bin/bash", (word)"after-exec", 0}, exec_env[] = {0};
        print_result("GINUX_FIXTURE: exec=",
                     call3(59, (word)"/usr/bin/bash", (word)exec_argv, (word)exec_env));
        return 0;
    }
    default:
        return 81;
    }
}

__asm__(".global _start\n"
        "_start:\n"
        "mov %rsp, %rdi\n"
        "and $-16, %rsp\n"
        "call native_fixture_main\n"
        "mov %rax, %rdi\n"
        "mov $231, %rax\n"
        "syscall\n");

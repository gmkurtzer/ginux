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

long native_fixture_main(void)
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
    case 't': {
        /* A plain SIGTRAP stop between syscalls; uname stays denied before and
         * after it unless supervision has lost track of entry and exit stops. */
        char buffer[6 * 65];
        print_result("GINUX_FIXTURE: uname-before=", call3(63, (word)buffer, 0, 0));
        __asm__ volatile("int3");
        print_result("GINUX_FIXTURE: uname-after=", call3(63, (word)buffer, 0, 0));
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
    default:
        return 81;
    }
}

__asm__(".global _start\n"
        "_start:\n"
        "and $-16, %rsp\n"
        "call native_fixture_main\n"
        "mov %rax, %rdi\n"
        "mov $231, %rax\n"
        "syscall\n");

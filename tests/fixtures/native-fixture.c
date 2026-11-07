/* Freestanding x86-64 workload installed as /usr/bin/bash for `ginux native`
 * supervision checks. `native` always runs bash with fixed arguments, so the
 * scenario is read from /scenario in the instance root. */
typedef unsigned long word;

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

static int sigchld_pipe = -1;
static volatile int sigchld_seen;

static void sigchld_handler(int signal)
{
    (void)signal;
    sigchld_seen = 1;
    call3(1, sigchld_pipe, (word)"c", 1);
}

void sigchld_restorer(void);
__asm__(".global sigchld_restorer\n"
        "sigchld_restorer:\n"
        "mov $15, %rax\n"
        "syscall\n");

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
        int status = 0;
        long pid;
        if (call4(293, (word)fds, 0, 0, 0) < 0) return 82;
        sigchld_pipe = fds[1];
        print_result("GINUX_FIXTURE: sigaction=", call4(13, 17, (word)&action, 0, 8));
        pid = call3(57, 0, 0, 0);
        if (pid == 0) return 7;
        print_result("GINUX_FIXTURE: fork-ok=", pid > 0);
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
    case 'r': {
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

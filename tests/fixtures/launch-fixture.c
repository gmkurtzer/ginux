/* Freestanding x86-64 workload for end-to-end launch checks. */
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

/* Signal return trampoline (SA_RESTORER), as libc provides it. */
void launch_fixture_restore(void);
__asm__(".global launch_fixture_restore\n"
        "launch_fixture_restore:\n"
        "mov $15, %eax\n"
        "syscall\n"
        "ud2\n");

static volatile int handled;

static void on_signal(int signal)
{
    (void)signal;
    handled = 1;
}

static word length(const char *value)
{
    word count = 0;
    while (value[count]) ++count;
    return count;
}

static void print(const char *value)
{
    call3(1, 1, (word)value, length(value));
    call3(1, 1, (word)"\n", 1);
}

/* "label<decimal value>\n" in one write. */
static void print_number(const char *label, long value)
{
    char line[64];
    char digits[24];
    word used = 0, at = sizeof digits;
    word magnitude = value < 0 ? -(word)value : (word)value;
    do digits[--at] = '0' + magnitude % 10; while (magnitude /= 10);
    if (value < 0) digits[--at] = '-';
    while (*label && used < sizeof line - 26) line[used++] = *label++;
    while (at < sizeof digits) line[used++] = digits[at++];
    line[used++] = '\n';
    call3(1, 1, (word)line, used);
}

static word number(const char *value)
{
    word result = 0;
    while (*value) result = result * 10 + (*value++ - '0');
    return result;
}

long launch_fixture_main(word *stack)
{
    word argc = *stack;
    char **argv = (char **)(stack + 1);
    char **env = argv + argc + 1;
    if (argc < 2) return 90;
    switch (argv[1][0]) {
    case 'e': return argc > 2 ? number(argv[2]) : 0;
    case 'a':
        for (word i = 2; i < argc; ++i) print(argv[i]);
        while (*env) print(*env++);
        return 0;
    case 'c': {
        char buffer[64];
        long fd = call3(2, (word)"input", 0, 0);
        if (fd < 0) return 91;
        long count = call3(0, fd, (word)buffer, sizeof buffer);
        if (count < 0) return 92;
        call3(1, 1, (word)buffer, count);
        return 0;
    }
    case 't': __asm__ volatile("int3"); return 93;
    case 's': __asm__ volatile("xor %%rax, %%rax; mov (%%rax), %%rax" ::: "rax", "memory"); return 94;
    case 'f': return call3(57, 0, 0, 0) == -38 ? 0 : 95;
    case 'i':
        __asm__ volatile("int $0x80" :: "a"(20), "b"(1), "c"(0), "d"(0) : "memory");
        return 96;
    case 'd': {
        char byte;
        word fd = argc > 2 ? number(argv[2]) : 3;
        return call3(0, fd, (word)&byte, 1) == -9 ? 0 : 97;
    }
    case 'w':
        print("ready");
        for (;;) __asm__ volatile("pause");
    case 'b':
        for (word i = 0; i < 4000; ++i) call3(39, 0, 0, 0);
        return 0;
    case 'h': {
        /* SIGTRAP handler installed with rt_sigaction; it must return through
         * rt_sigreturn and resume after the int3. */
        struct { word handler, flags, restorer, mask; } action = {
            (word)on_signal, 0x04000000 /* SA_RESTORER */, (word)launch_fixture_restore, 0};
        if (call4(13, 5, (word)&action, 0, 8) != 0) return 86;
        __asm__ volatile("int3");
        if (!handled) return 87;
        print("handled");
        return 0;
    }
    case 'l': {
        /* opendir("."): openat(AT_FDCWD, ".", O_RDONLY|O_NONBLOCK|O_CLOEXEC|O_DIRECTORY) */
        char buffer[1024];
        long fd = call3(257, (word)-100, (word)".", 0204000 | 02000000);
        if (fd < 0) return 81;
        long count = call3(217, fd, (word)buffer, sizeof buffer);
        if (count <= 0) return 82;
        /* struct linux_dirent64: d_ino 8, d_off 8, d_reclen 2, d_type 1, d_name */
        for (long at = 0; at < count; at += *(unsigned short *)(buffer + at + 16)) {
            const char *name = buffer + at + 19;
            if (name[0] == 'i' && name[1] == 'n' && name[2] == 'p' && name[3] == 'u'
                && name[4] == 't' && name[5] == 0) {
                print(name);
                return 0;
            }
        }
        return 83;
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
        print_number("set_tid_address=", call3(218, (word)&clear_tid, 0, 0));
        print_number("getpid=", call3(39, 0, 0, 0));
        print_number("gettid=", call3(186, 0, 0, 0));
        print_number("futex_lock_pi=", call4(202, (word)&pi_word, 6 | 128, 0, 0));
        print_number("pi_word=", pi_word);
        print_number("futex_lock_pi_flags=",
                     call4(202, (word)&pi_flags_word, 6 | 128 | 1024, 0, 0));
        print_number("pi_flags_word=", pi_flags_word);
        return 0;
    }
    case 'o': {
        static char input[1024 * 1024 + 1];
        static char output[4097];
        long fd = call3(2, (word)"large", 0, 0);
        if (fd < 0) return 84;
        print_number("read=", call3(0, fd, (word)input, sizeof input));
        print_number("pread=", call4(17, fd, (word)input, sizeof input, 0));
        for (word i = 0; i < sizeof output; ++i) output[i] = 'x';
        long written = call3(1, 1, (word)output, sizeof output);
        print_number("write=", written);
        return 0;
    }
    case 'r': {
        static char random[512];
        print_number("getrandom=", call3(318, (word)random, sizeof random, 0));
        return 0;
    }
    default: return 98;
    }
}

__asm__(".global _start\n"
        "_start:\n"
        "mov %rsp, %rdi\n"
        "and $-16, %rsp\n"
        "call launch_fixture_main\n"
        "mov %rax, %rdi\n"
        "mov $60, %rax\n"
        "syscall\n");

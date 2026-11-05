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

/* Freestanding x86-64 programs for the Gate 1 ptrace setup failure check.
 *
 * Built with -DDENY_PTRACE: `deny-ptrace <request> <program> [args...]`
 * installs a seccomp filter that fails ptrace(<request>, ...) with EPERM and
 * executes <program> with the remaining arguments; `deny-ptrace all ...` fails
 * every ptrace request except PTRACE_TRACEME. The filter is inherited by
 * Gate 1's tracee, which only uses PTRACE_TRACEME, so only the supervisor's
 * request fails.
 *
 * Built without it: a payload that records that it ran by creating
 * /var/tmp/gate1-payload-ran. Installed as /var/tmp/ginux-managed-payload, it
 * shows whether the payload got past the stop after execve. */
typedef unsigned long word;

static long call3(long number, word a, word b, word c)
{
    long result;
    __asm__ volatile("syscall" : "=a"(result)
                     : "a"(number), "D"(a), "S"(b), "d"(c)
                     : "rcx", "r11", "memory");
    return result;
}

#ifdef DENY_PTRACE
/* prctl checks the unused arguments of PR_SET_NO_NEW_PRIVS, so pass zeros. */
static long prctl(word option, word a, word b)
{
    long result;
    register word r10 __asm__("r10") = 0;
    register word r8 __asm__("r8") = 0;
    __asm__ volatile("syscall" : "=a"(result)
                     : "a"(157L), "D"(option), "S"(a), "d"(b), "r"(r10), "r"(r8)
                     : "rcx", "r11", "memory");
    return result;
}

struct filter {
    unsigned short code;
    unsigned char jt, jf;
    unsigned int k;
};

struct program {
    unsigned short length;
    struct filter *filter;
};

static word parse(const char *text)
{
    word value = 0;
    while (*text >= '0' && *text <= '9') value = value * 10 + (word)(*text++ - '0');
    return value;
}

long fixture_main(long argc, char **argv, char **envp)
{
    if (argc < 3) return 90;
    static struct filter filter[] = {
        {0x20, 0, 0, 4},              /* ld seccomp_data.arch */
        {0x15, 0, 5, 0xc000003e},     /* jeq AUDIT_ARCH_X86_64 */
        {0x20, 0, 0, 0},              /* ld seccomp_data.nr */
        {0x15, 0, 3, 101},            /* jeq ptrace */
        {0x20, 0, 0, 16},             /* ld seccomp_data.args[0] (low word) */
        {0x15, 0, 1, 0},              /* jeq <request>, set below */
        {0x06, 0, 0, 0x00050000 | 1}, /* ret SECCOMP_RET_ERRNO | EPERM */
        {0x06, 0, 0, 0x7fff0000},     /* ret SECCOMP_RET_ALLOW */
    };
    if (argv[1][0] == 'a') {
        /* all: allow PTRACE_TRACEME (0), deny every other request. */
        filter[5].jt = 1;
        filter[5].jf = 0;
    } else {
        filter[5].k = (unsigned int)parse(argv[1]);
    }
    struct program program = {sizeof filter / sizeof filter[0], filter};
    /* PR_SET_NO_NEW_PRIVS, then PR_SET_SECCOMP with SECCOMP_MODE_FILTER. */
    if (prctl(38, 1, 0) < 0 || prctl(22, 2, (word)&program) < 0) return 91;
    call3(59, (word)argv[2], (word)(argv + 2), (word)envp);
    return 92;
}
#else
long fixture_main(long argc, char **argv, char **envp)
{
    (void)argc;
    (void)argv;
    (void)envp;
    long fd = call3(2, (word)"/var/tmp/gate1-payload-ran", 0101 /* O_WRONLY|O_CREAT */, 0644);
    if (fd < 0) return 1;
    call3(1, fd, (word)"ran\n", 4);
    call3(3, fd, 0, 0);
    return 0;
}
#endif

__asm__(".global _start\n"
        "_start:\n"
        "mov (%rsp), %rdi\n"
        "lea 8(%rsp), %rsi\n"
        "lea 16(%rsp,%rdi,8), %rdx\n"
        "and $-16, %rsp\n"
        "call fixture_main\n"
        "mov %rax, %rdi\n"
        "mov $231, %rax\n"
        "syscall\n");

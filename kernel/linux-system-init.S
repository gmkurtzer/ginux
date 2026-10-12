.intel_syntax noprefix

.section .text
.global ginux_linux_system_init
.type ginux_linux_system_init, @function
ginux_linux_system_init:
    mov eax, 39
    syscall
    cmp rax, 1
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lconsole]
    mov edx, 1
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r8, rax
    mov rdi, rax
    lea rsi, [rip + .Lready]
    mov edx, 21
    mov eax, 1
    syscall
    cmp rax, 21
    jne .Lfailure
    mov rdi, r8
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, 42
    mov eax, 231
    syscall

.Lfailure:
    mov edi, 1
    mov eax, 231
    syscall
    ud2

.section .data
.align 8
.Lready:
    .ascii "GINUX_SYSTEM_INIT_V0\n"
.Lready_end:
.Lconsole:
    .asciz "/dev/console"

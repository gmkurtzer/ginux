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

    mov edi, -100
    lea rsi, [rip + .Lproc_path]
    mov edx, 0x10000
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 80
    mov eax, 217
    syscall
    cmp rax, 72
    jne .Lfailure
    cmp byte ptr [rip + .Lstatus + 67], '1'
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lpid1_path]
    mov edx, 0x10000
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 80
    mov eax, 217
    syscall
    cmp rax, 80
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 67], 0x74617473
    jne .Lfailure
    cmp word ptr [rip + .Lstatus + 71], 0x7375
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lstatus_path]
    xor edx, edx
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 45
    xor eax, eax
    syscall
    cmp rax, 45
    jne .Lfailure
    mov rdi, r8
    lea rsi, [rip + .Lstatus]
    mov edx, 45
    mov eax, 1
    syscall
    cmp rax, 45
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov rdi, r8
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
.Lstatus_path:
    .asciz "/proc/1/status"
.Lproc_path:
    .asciz "/proc"
.Lpid1_path:
    .asciz "/proc/1"
.Lstatus:
    .space 80

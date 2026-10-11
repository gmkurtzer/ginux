.intel_syntax noprefix

.section .text
.global ginux_linux_exec_target
.type ginux_linux_exec_target, @function
ginux_linux_exec_target:
    cmp qword ptr [rsp], 2
    jne .Lfailure
    mov rax, qword ptr [rsp + 8]
    movabs rcx, 0x6578652f6e69622f
    cmp qword ptr [rax], rcx
    jne .Lfailure
    movabs rcx, 0x7465677261742d63
    cmp qword ptr [rax + 8], rcx
    jne .Lfailure
    cmp byte ptr [rax + 16], 0
    jne .Lfailure
    mov rax, qword ptr [rsp + 16]
    movabs rcx, 0x7261742d63657865
    cmp qword ptr [rax], rcx
    jne .Lfailure
    cmp dword ptr [rax + 8], 0x00746567
    jne .Lfailure
    cmp qword ptr [rsp + 24], 0
    jne .Lfailure
    mov rax, qword ptr [rsp + 32]
    movabs rcx, 0x58455f58554e4947
    cmp qword ptr [rax], rcx
    jne .Lfailure
    cmp dword ptr [rax + 8], 0x313d4345
    jne .Lfailure
    cmp byte ptr [rax + 12], 0
    jne .Lfailure
    cmp qword ptr [rsp + 40], 0
    jne .Lfailure
    cmp qword ptr [rsp + 48], 0
    jne .Lfailure
    cmp qword ptr [rsp + 56], 0
    jne .Lfailure
    mov edi, -100
    lea rsi, [rip + .Lconsole]
    mov edx, 1
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r8, rax
    lea rsi, [rip + .Lmessage]
    mov edi, eax
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
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
    mov edi, 127
    mov eax, 231
    syscall
    ud2

.Lmessage:
    .ascii "EXEC_OK\n"
.Lconsole:
    .asciz "/dev/console"
.size ginux_linux_exec_target, . - ginux_linux_exec_target

.section .data
.quad 0x4558454354415247

.section .note.GNU-stack,"",@progbits

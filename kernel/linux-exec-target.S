.intel_syntax noprefix

.section .text
.global ginux_linux_exec_target
.type ginux_linux_exec_target, @function
ginux_linux_exec_target:
    cmp qword ptr [rsp], 2
    jne .Lfailure
    mov rax, qword ptr [rsp + 16]
    movabs rcx, 0x7261742d63657865
    cmp qword ptr [rax], rcx
    jne .Lfailure
    cmp dword ptr [rax + 8], 0x00746567
    jne .Lfailure
    lea rsi, [rip + .Lmessage]
    mov edi, 1
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
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
.size ginux_linux_exec_target, . - ginux_linux_exec_target

.section .data
.quad 0x4558454354415247

.section .note.GNU-stack,"",@progbits

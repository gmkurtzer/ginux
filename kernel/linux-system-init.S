.intel_syntax noprefix

.section .text
.global ginux_linux_system_init
.type ginux_linux_system_init, @function
ginux_linux_system_init:
    cmp qword ptr [rsp], 1
    jne .Lfailure
    mov rax, qword ptr [rsp + 8]
    movabs rcx, 0x6e692f6e6962732f
    cmp qword ptr [rax], rcx
    jne .Lfailure
    cmp dword ptr [rax + 8], 0x007469
    jne .Lfailure
    cmp qword ptr [rsp + 16], 0
    jne .Lfailure

    mov eax, 39
    syscall
    cmp rax, 1
    jne .Lfailure

    mov edi, 1
    lea rsi, [rip + .Lready]
    mov edx, 21
    mov eax, 1
    syscall
    cmp rax, 21
    jne .Lfailure

    mov edi, 42
    mov eax, 231
    syscall

.Lfailure:
    mov edi, 1
    mov eax, 231
    syscall
    ud2

.Lready:
    .ascii "GINUX_SYSTEM_INIT_V0\n"
.Lready_end:

.section .data
.align 8
.quad 0

.intel_syntax noprefix

.section .text
.global ginux_linux_conformance
.type ginux_linux_conformance, @function
ginux_linux_conformance:
    lea rdi, [rip + .Lmemfd_name]
    mov esi, 2
    mov eax, 319
    syscall
    test rax, rax
    js .Lfailure
    mov r12d, eax

    mov edi, r12d
    mov eax, 32
    syscall
    test rax, rax
    js .Lfailure
    mov r13d, eax

    mov edi, r12d
    lea rsi, [rip + .Lpayload]
    mov edx, 4
    mov eax, 1
    syscall
    cmp rax, 4
    jne .Lfailure

    mov edi, r13d
    mov esi, 1033
    mov edx, 4
    mov eax, 72
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, r12d
    mov esi, 1034
    xor edx, edx
    mov eax, 72
    syscall
    cmp rax, 4
    jne .Lfailure

    mov edi, r12d
    lea rsi, [rip + .Lpayload]
    mov edx, 1
    mov r10d, 4
    mov eax, 18
    syscall
    cmp rax, -1
    jne .Lfailure
    mov edi, r12d
    mov esi, 2
    mov eax, 77
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, r12d
    mov esi, 5
    mov eax, 77
    syscall
    cmp rax, -1
    jne .Lfailure

    mov edi, r12d
    mov esi, 1033
    mov edx, 0x2a
    mov eax, 72
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, r12d
    mov esi, 1
    mov eax, 77
    syscall
    cmp rax, -1
    jne .Lfailure
    mov edi, r12d
    lea rsi, [rip + .Lpayload]
    mov edx, 1
    xor r10d, r10d
    mov eax, 18
    syscall
    cmp rax, -1
    jne .Lfailure
    mov edi, r12d
    mov esi, 0666
    mov eax, 91
    syscall
    cmp rax, -1
    jne .Lfailure

    mov edi, r12d
    mov esi, 1033
    mov edx, 1
    mov eax, 72
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, r12d
    mov esi, 1033
    mov edx, 0x10
    mov eax, 72
    syscall
    cmp rax, -1
    jne .Lfailure
    mov edi, r13d
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, r12d
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    lea rdi, [rip + .Lmemfd_name]
    xor esi, esi
    mov eax, 319
    syscall
    test rax, rax
    js .Lfailure
    mov r12d, eax
    mov edi, r12d
    mov esi, 1034
    xor edx, edx
    mov eax, 72
    syscall
    cmp rax, 1
    jne .Lfailure
    mov edi, r12d
    mov esi, 1033
    mov edx, 4
    mov eax, 72
    syscall
    cmp rax, -1
    jne .Lfailure
    mov edi, r12d
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lconsole]
    mov edx, 1
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r12, rax
    mov rdi, rax
    lea rsi, [rip + .Lsuccess]
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne .Lfailure
    mov rdi, r12
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    xor edi, edi
    mov eax, 231
    syscall

.Lfailure:
    mov edi, 127
    mov eax, 231
    syscall
    ud2

.Lmemfd_name:
    .asciz "ginux-seals"
.Lconsole:
    .asciz "/dev/console"
.Lpayload:
    .ascii "data"
.Lsuccess:
    .ascii "ABI_OK!\n"
.size ginux_linux_conformance, . - ginux_linux_conformance

.section .data
.quad 0x434e464f524d414e

.section .note.GNU-stack,"",@progbits

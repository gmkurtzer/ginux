.intel_syntax noprefix

.section .text
.global ginux_linux_exec_target
.type ginux_linux_exec_target, @function
ginux_linux_exec_target:
    cmp qword ptr [rsp], 2
    jne .Lfailure
    mov rax, qword ptr [rsp + 8]
#ifdef GINUX_SIGNAL_PROBE
    movabs rcx, 0x6769732f6e69622f
#else
    movabs rcx, 0x6578652f6e69622f
#endif
    cmp qword ptr [rax], rcx
    jne .Lfailure
#ifdef GINUX_EXEC_SECOND
    movabs rcx, 0x646e6f6365732d63
#elif defined(GINUX_SIGNAL_PROBE)
    movabs rcx, 0x626f72702d6c616e
#else
    movabs rcx, 0x7465677261742d63
#endif
    cmp qword ptr [rax + 8], rcx
    jne .Lfailure
#ifdef GINUX_SIGNAL_PROBE
    cmp word ptr [rax + 16], 0x65
#else
    cmp byte ptr [rax + 16], 0
#endif
    jne .Lfailure
    mov rax, qword ptr [rsp + 16]
#ifdef GINUX_EXEC_SECOND
    movabs rcx, 0x6365732d63657865
#elif defined(GINUX_SIGNAL_PROBE)
    movabs rcx, 0x702d6c616e676973
#else
    movabs rcx, 0x7261742d63657865
#endif
    cmp qword ptr [rax], rcx
    jne .Lfailure
#ifdef GINUX_EXEC_SECOND
    cmp dword ptr [rax + 8], 0x00646e6f
#elif defined(GINUX_SIGNAL_PROBE)
    cmp dword ptr [rax + 8], 0x65626f72
#else
    cmp dword ptr [rax + 8], 0x00746567
#endif
    jne .Lfailure
#ifdef GINUX_SIGNAL_PROBE
    cmp byte ptr [rax + 12], 0
    jne .Lfailure
#endif
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
    cmp qword ptr [rsp + 48], 3
    jne .Lfailure
    cmp qword ptr [rsp + 56], 0
    je .Lfailure
    cmp qword ptr [rsp + 64], 4
    jne .Lfailure
    cmp qword ptr [rsp + 72], 56
    jne .Lfailure
    cmp qword ptr [rsp + 80], 5
    jne .Lfailure
    cmp qword ptr [rsp + 88], 5
    jne .Lfailure
    cmp qword ptr [rsp + 96], 6
    jne .Lfailure
    cmp qword ptr [rsp + 104], 4096
    jne .Lfailure
    cmp qword ptr [rsp + 112], 7
    jne .Lfailure
    cmp qword ptr [rsp + 120], 0
    jne .Lfailure
    cmp qword ptr [rsp + 128], 8
    jne .Lfailure
    cmp qword ptr [rsp + 136], 0
    jne .Lfailure
    cmp qword ptr [rsp + 144], 11
    jne .Lfailure
    cmp qword ptr [rsp + 152], 0
    jne .Lfailure
    cmp qword ptr [rsp + 160], 12
    jne .Lfailure
    cmp qword ptr [rsp + 168], 0
    jne .Lfailure
    cmp qword ptr [rsp + 176], 13
    jne .Lfailure
    cmp qword ptr [rsp + 184], 0
    jne .Lfailure
    cmp qword ptr [rsp + 192], 14
    jne .Lfailure
    cmp qword ptr [rsp + 200], 0
    jne .Lfailure
    cmp qword ptr [rsp + 208], 23
    jne .Lfailure
    cmp qword ptr [rsp + 216], 0
    jne .Lfailure
    cmp qword ptr [rsp + 224], 31
    jne .Lfailure
    mov rax, qword ptr [rsp + 232]
    cmp rax, qword ptr [rsp + 8]
    jne .Lfailure
    cmp qword ptr [rsp + 240], 9
    jne .Lfailure
#ifdef GINUX_EXEC_SECOND
    cmp qword ptr [rsp + 248], 0x410100
#else
    cmp qword ptr [rsp + 248], 0x400100
#endif
    jne .Lfailure
    cmp qword ptr [rsp + 256], 17
    jne .Lfailure
    cmp qword ptr [rsp + 264], 100
    jne .Lfailure
    cmp qword ptr [rsp + 272], 15
    jne .Lfailure
    mov rax, qword ptr [rsp + 280]
    cmp dword ptr [rax], 0x5f363878
    jne .Lfailure
    cmp word ptr [rax + 4], 0x3436
    jne .Lfailure
    cmp byte ptr [rax + 6], 0
    jne .Lfailure
    cmp qword ptr [rsp + 288], 16
    jne .Lfailure
    cmp qword ptr [rsp + 296], 0
    jne .Lfailure
    cmp qword ptr [rsp + 304], 26
    jne .Lfailure
    cmp qword ptr [rsp + 312], 0
    jne .Lfailure
    cmp qword ptr [rsp + 320], 0
    jne .Lfailure
    cmp qword ptr [rsp + 328], 0
    jne .Lfailure

    mov edi, 2
    mov esi, 1
    xor edx, edx
    mov eax, 72
    syscall
    cmp rax, -9
    jne .Lfailure
    mov edi, 3
    mov esi, 1
    xor edx, edx
    mov eax, 72
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, 10
    xor esi, esi
    lea rdx, [rip + .Lsignal_action]
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_action], 0
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_action + 8], 0
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_action + 16], 0
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_action + 24], 0
    jne .Lfailure
    mov edi, 12
    xor esi, esi
    lea rdx, [rip + .Lsignal_action]
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_action], 1
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_action + 8], 0
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_action + 16], 0
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_action + 24], 0
    jne .Lfailure

    xor edi, edi
    lea rsi, [rip + .Lalt_stack]
    mov eax, 131
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lalt_stack], 0
    jne .Lfailure
    cmp dword ptr [rip + .Lalt_stack + 8], 2
    jne .Lfailure
    cmp qword ptr [rip + .Lalt_stack + 16], 0
    jne .Lfailure

#ifdef GINUX_SIGNAL_PROBE
    lea rax, [rip + .Lsignal_handler]
    mov qword ptr [rip + .Lsignal_action], rax
    mov qword ptr [rip + .Lsignal_action + 8], 0x04000000
    lea rax, [rip + .Lsignal_restorer]
    mov qword ptr [rip + .Lsignal_action + 16], rax
    mov qword ptr [rip + .Lsignal_action + 24], 0
    mov edi, 11
    lea rsi, [rip + .Lsignal_action]
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne .Lfailure

    xor edi, edi
    mov esi, 1
    mov edx, 11
    mov eax, 234
    syscall
    cmp rax, -22
    jne .Lfailure
    mov edi, -1
    mov esi, 1
    mov edx, 11
    mov eax, 234
    syscall
    cmp rax, -22
    jne .Lfailure
    mov edi, 1
    xor esi, esi
    mov edx, 11
    mov eax, 234
    syscall
    cmp rax, -22
    jne .Lfailure
    mov edi, 2
    mov esi, 1
    mov edx, 11
    mov eax, 234
    syscall
    cmp rax, -3
    jne .Lfailure
    mov edi, 1
    mov esi, 2
    mov edx, 11
    mov eax, 234
    syscall
    cmp rax, -3
    jne .Lfailure
    mov edi, 1
    mov esi, 1
    mov edx, 65
    mov eax, 234
    syscall
    cmp rax, -22
    jne .Lfailure
    mov edi, 1
    mov esi, 1
    xor edx, edx
    mov eax, 234
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, 1
    mov esi, 1
    mov edx, 11
    mov eax, 234
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_count], 1
    jne .Lfailure

    xor edi, edi
    mov esi, 11
    mov eax, 200
    syscall
    cmp rax, -22
    jne .Lfailure
    mov edi, -1
    mov esi, 11
    mov eax, 200
    syscall
    cmp rax, -22
    jne .Lfailure
    mov edi, 2
    mov esi, 11
    mov eax, 200
    syscall
    cmp rax, -3
    jne .Lfailure
    mov edi, 1
    mov esi, 65
    mov eax, 200
    syscall
    cmp rax, -22
    jne .Lfailure
    mov edi, 1
    xor esi, esi
    mov eax, 200
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, 1
    mov esi, 11
    mov eax, 200
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_count], 2
    jne .Lfailure

    mov qword ptr [rip + .Lsignal_mask], 0x400
    xor edi, edi
    lea rsi, [rip + .Lsignal_mask]
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, 1
    mov esi, 1
    mov edx, 11
    mov eax, 234
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, 1
    mov esi, 11
    mov eax, 200
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_count], 2
    jne .Lfailure

    lea rdi, [rip + .Lpending_mask]
    mov esi, 7
    mov eax, 127
    syscall
    cmp rax, -22
    jne .Lfailure
    mov edi, 1
    mov esi, 8
    mov eax, 127
    syscall
    cmp rax, -14
    jne .Lfailure
    lea rdi, [rip + .Lpending_mask]
    mov esi, 8
    mov eax, 127
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lpending_mask], 0x400
    jne .Lfailure

    mov edi, 1
    lea rsi, [rip + .Lsignal_mask]
    lea rdx, [rip + .Lold_signal_mask]
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_count], 3
    jne .Lfailure
    cmp qword ptr [rip + .Lold_signal_mask], 0x400
    jne .Lfailure
    lea rdi, [rip + .Lpending_mask]
    mov esi, 8
    mov eax, 127
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lpending_mask], 0
    jne .Lfailure

    mov qword ptr [rip + .Lsignal_action], 0
    mov qword ptr [rip + .Lsignal_action + 8], 0
    mov qword ptr [rip + .Lsignal_action + 16], 0
    mov qword ptr [rip + .Lsignal_action + 24], 0
    mov edi, 11
    lea rsi, [rip + .Lsignal_action]
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne .Lfailure
#endif

    mov edi, 16
    lea rsi, [rip + .Lprocess_name]
    mov eax, 157
    syscall
    test rax, rax
    jne .Lfailure
#ifdef GINUX_EXEC_SECOND
    movabs rcx, 0x6365732d63657865
#elif defined(GINUX_SIGNAL_PROBE)
    movabs rcx, 0x702d6c616e676973
#else
    movabs rcx, 0x7261742d63657865
#endif
    cmp qword ptr [rip + .Lprocess_name], rcx
    jne .Lfailure
#ifdef GINUX_EXEC_SECOND
    cmp dword ptr [rip + .Lprocess_name + 8], 0x00646e6f
#elif defined(GINUX_SIGNAL_PROBE)
    cmp dword ptr [rip + .Lprocess_name + 8], 0x65626f72
#else
    cmp dword ptr [rip + .Lprocess_name + 8], 0x00746567
#endif
    jne .Lfailure

    lea rdi, [rip + .Lexecutable_link]
    lea rsi, [rip + .Lexecutable_path]
    mov edx, 32
    mov eax, 89
    syscall
#ifdef GINUX_SIGNAL_PROBE
    cmp rax, 17
#else
    cmp rax, 16
#endif
    jne .Lfailure
#ifdef GINUX_SIGNAL_PROBE
    movabs rcx, 0x6769732f6e69622f
#else
    movabs rcx, 0x6578652f6e69622f
#endif
    cmp qword ptr [rip + .Lexecutable_path], rcx
    jne .Lfailure
#ifdef GINUX_EXEC_SECOND
    movabs rcx, 0x646e6f6365732d63
#elif defined(GINUX_SIGNAL_PROBE)
    movabs rcx, 0x626f72702d6c616e
#else
    movabs rcx, 0x7465677261742d63
#endif
    cmp qword ptr [rip + .Lexecutable_path + 8], rcx
    jne .Lfailure

    mov edi, 3
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

#ifdef GINUX_SIGNAL_PROBE
.Lsignal_handler:
    cmp edi, 11
    jne .Lfailure
    test rsi, rsi
    jne .Lfailure
    test rdx, rdx
    jne .Lfailure
    inc qword ptr [rip + .Lsignal_count]
    ret

.Lsignal_restorer:
    mov eax, 15
    syscall
    ud2
#endif

.Lmessage:
#ifdef GINUX_EXEC_SECOND
    .ascii "SECOND!\n"
#elif defined(GINUX_SIGNAL_PROBE)
    .ascii "SIGNAL!\n"
#else
    .ascii "EXEC_OK\n"
#endif
.Lconsole:
    .asciz "/dev/console"
.Lexecutable_link:
    .asciz "/proc/self/exe"
.size ginux_linux_exec_target, . - ginux_linux_exec_target

#ifdef GINUX_EXEC_SECOND
.section .text.exec_tail,"ax",@progbits
.balign 16
.Lsecond_exec_page_proof:
    .fill 4096, 1, 0x90
#endif

.section .data
#ifdef GINUX_EXEC_SECOND
.quad 0x5345434f4e444152
#else
.quad 0x4558454354415247
#endif

.section .bss
.balign 8
.Lprocess_name:
    .zero 16
.Lsignal_action:
    .zero 32
.Lalt_stack:
    .zero 24
.Lexecutable_path:
    .zero 32
#ifdef GINUX_SIGNAL_PROBE
.Lsignal_count:
    .zero 8
.Lsignal_mask:
    .zero 8
.Lold_signal_mask:
    .zero 8
.Lpending_mask:
    .zero 8
#endif

.section .note.GNU-stack,"",@progbits

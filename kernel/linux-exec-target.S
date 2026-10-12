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
    mov qword ptr [rip + .Lalt_stack], 0x402800
    mov dword ptr [rip + .Lalt_stack + 8], 0x80000000
    mov qword ptr [rip + .Lalt_stack + 16], 2048
    lea rdi, [rip + .Lalt_stack]
    xor esi, esi
    mov eax, 131
    syscall
    test rax, rax
    jne .Lfailure

    lea rax, [rip + .Lsignal_handler]
    mov qword ptr [rip + .Lsignal_action], rax
    mov qword ptr [rip + .Lsignal_action + 8], 0x0c000004
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

    mov qword ptr [rip + .Lsignal_mode], 2
    cmp qword ptr [0x40b000], 0
    jne .Lfailure
    mov qword ptr [rip + .Lsignal_mode], 0
    cmp qword ptr [rip + .Lsignal_count], 1
    jne .Lfailure
    xor edi, edi
    lea rsi, [rip + .Lalt_stack]
    mov eax, 131
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lalt_stack], 0x402800
    jne .Lfailure
    cmp dword ptr [rip + .Lalt_stack + 8], 0x80000000
    jne .Lfailure
    cmp qword ptr [rip + .Lalt_stack + 16], 2048
    jne .Lfailure
    mov qword ptr [rip + .Lalt_stack], 0
    mov dword ptr [rip + .Lalt_stack + 8], 2
    mov qword ptr [rip + .Lalt_stack + 16], 0
    lea rdi, [rip + .Lalt_stack]
    xor esi, esi
    mov eax, 131
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, 0x40b000
    mov esi, 4096
    mov eax, 11
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
    cmp qword ptr [rip + .Lsignal_count], 2
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
    cmp qword ptr [rip + .Lsignal_count], 3
    jne .Lfailure

    lea rax, [rip + .Lsignal_handler]
    mov qword ptr [rip + .Lsignal_action], rax
    mov qword ptr [rip + .Lsignal_action + 8], 0x04000004
    lea rax, [rip + .Lsignal_restorer]
    mov qword ptr [rip + .Lsignal_action + 16], rax
    mov qword ptr [rip + .Lsignal_action + 24], 0
    mov edi, 10
    lea rsi, [rip + .Lsignal_action]
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne .Lfailure
    mov qword ptr [rip + .Lsignal_mode], 3
    mov qword ptr [rip + .Lexpected_signal], 10
    mov qword ptr [rip + .Lexpected_signal_code], 0
    mov qword ptr [rip + .Lexpected_signal_mask], 0x200
    mov edi, 1
    mov esi, 10
    mov eax, 62
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_count], 4
    jne .Lfailure

    mov qword ptr [rip + .Lsignal_mask], 0x200
    xor edi, edi
    lea rsi, [rip + .Lsignal_mask]
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, 1
    mov esi, 10
    mov eax, 62
    syscall
    test rax, rax
    jne .Lfailure
    lea rdi, [rip + .Lsignal_mask]
    lea rsi, [rip + .Lwait_info]
    lea rdx, [rip + .Lwait_timeout]
    mov r10d, 7
    mov eax, 128
    syscall
    cmp rax, -22
    jne .Lfailure
    mov edi, 1
    xor esi, esi
    lea rdx, [rip + .Lwait_timeout]
    mov r10d, 8
    mov eax, 128
    syscall
    cmp rax, -14
    jne .Lfailure
    mov qword ptr [rip + .Lwait_timeout + 8], 1000000000
    lea rdi, [rip + .Lsignal_mask]
    xor esi, esi
    lea rdx, [rip + .Lwait_timeout]
    mov r10d, 8
    mov eax, 128
    syscall
    cmp rax, -22
    jne .Lfailure
    mov qword ptr [rip + .Lwait_timeout + 8], 0
    lea rdi, [rip + .Lsignal_mask]
    mov esi, 1
    lea rdx, [rip + .Lwait_timeout]
    mov r10d, 8
    mov eax, 128
    syscall
    cmp rax, -14
    jne .Lfailure
    lea rdi, [rip + .Lsignal_mask]
    lea rsi, [rip + .Lwait_info]
    lea rdx, [rip + .Lwait_timeout]
    mov r10d, 8
    mov eax, 128
    syscall
    cmp rax, 10
    jne .Lfailure
    cmp dword ptr [rip + .Lwait_info], 10
    jne .Lfailure
    cmp dword ptr [rip + .Lwait_info + 4], 0
    jne .Lfailure
    cmp dword ptr [rip + .Lwait_info + 8], 0
    jne .Lfailure
    cmp dword ptr [rip + .Lwait_info + 16], 1
    jne .Lfailure
    cmp dword ptr [rip + .Lwait_info + 20], 0
    jne .Lfailure
    mov eax, 128
    syscall
    cmp rax, -11
    jne .Lfailure
    xor edx, edx
    mov eax, 128
    syscall
    cmp rax, -38
    jne .Lfailure
    mov qword ptr [rip + .Lsignal_mask], 0
    mov edi, 2
    lea rsi, [rip + .Lsignal_mask]
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, 1
    mov esi, 12
    mov eax, 62
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_count], 4
    jne .Lfailure
    mov edi, 1
    mov esi, 15
    mov eax, 62
    syscall
    cmp rax, -38
    jne .Lfailure

    mov edi, 12
    lea rsi, [rip + .Lsignal_action]
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne .Lfailure
    mov qword ptr [rip + .Lsignal_mask], 0xa00
    xor edi, edi
    lea rsi, [rip + .Lsignal_mask]
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, 1
    mov esi, 12
    mov eax, 200
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, 1
    mov esi, 10
    mov eax, 62
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, 1
    mov esi, 1
    mov edx, 10
    mov eax, 234
    syscall
    test rax, rax
    jne .Lfailure
    lea rdi, [rip + .Lpending_mask]
    mov esi, 8
    mov eax, 127
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lpending_mask], 0xa00
    jne .Lfailure
    mov qword ptr [rip + .Lexpected_signal], 10
    mov qword ptr [rip + .Lexpected_signal_code], 0
    mov qword ptr [rip + .Lexpected_signal_mask], 0x200
    mov qword ptr [rip + .Lsignal_mask], 0xa00
    mov edi, 1
    lea rsi, [rip + .Lsignal_mask]
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    cmp rax, 0x5678
    jne .Lfailure
    lea rdi, [rip + .Lpending_mask]
    mov esi, 8
    mov eax, 127
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lpending_mask], 0
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_count], 6
    jne .Lfailure
    mov qword ptr [rip + .Lsignal_mode], 0
    lea rdi, [rip + .Lsignal_action]
    xor eax, eax
    mov ecx, 8
    rep stosd
    mov edi, 10
    lea rsi, [rip + .Lsignal_action]
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne .Lfailure
    mov qword ptr [rip + .Lsignal_action], 1
    mov edi, 12
    lea rsi, [rip + .Lsignal_action]
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
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
    cmp qword ptr [rip + .Lsignal_count], 6
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
    cmp qword ptr [rip + .Lsignal_count], 7
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

    lea rax, [rip + .Lsignal_handler]
    mov qword ptr [rip + .Lsignal_action], rax
    mov eax, 0xc4000004
    mov qword ptr [rip + .Lsignal_action + 8], rax
    lea rax, [rip + .Lsignal_restorer]
    mov qword ptr [rip + .Lsignal_action + 16], rax
    mov qword ptr [rip + .Lsignal_action + 24], 0x200
    mov edi, 11
    lea rsi, [rip + .Lsignal_action]
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne .Lfailure
    mov qword ptr [rip + .Lsignal_mode], 1
    mov edi, 1
    mov esi, 11
    mov eax, 200
    syscall
    cmp rax, 0x1234
    jne .Lfailure
    mov qword ptr [rip + .Lsignal_mode], 0
    cmp qword ptr [rip + .Lsignal_count], 8
    jne .Lfailure

    mov edi, 11
    xor esi, esi
    lea rdx, [rip + .Lold_signal_action]
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lold_signal_action], 0
    jne .Lfailure
    cmp qword ptr [rip + .Lold_signal_action + 8], 0
    jne .Lfailure
    cmp qword ptr [rip + .Lold_signal_action + 16], 0
    jne .Lfailure
    cmp qword ptr [rip + .Lold_signal_action + 24], 0
    jne .Lfailure
    mov edi, 99
    xor esi, esi
    lea rdx, [rip + .Lold_signal_mask]
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lold_signal_mask], 0x800
    jne .Lfailure
    mov qword ptr [rip + .Lsignal_mask], 0
    mov edi, 2
    lea rsi, [rip + .Lsignal_mask]
    xor edx, edx
    mov r10d, 8
    mov eax, 14
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
#ifdef GINUX_SIGNAL_PROBE
    mov qword ptr [rip + .Lsignal_action], 0
    mov edi, 13
    lea rsi, [rip + .Lsignal_action]
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne .Lfailure
    lea rdi, [rip + .Lpipe_descriptors]
    mov eax, 22
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, dword ptr [rip + .Lpipe_descriptors]
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, dword ptr [rip + .Lpipe_descriptors + 4]
    lea rsi, [rip + .Lmessage]
    mov edx, 1
    mov eax, 1
    syscall
    jmp .Lfailure
#else
    mov edi, 42
    mov eax, 231
    syscall
#endif

.Lfailure:
    mov edi, 127
    mov eax, 231
    syscall
    ud2

#ifdef GINUX_SIGNAL_PROBE
.Lsignal_handler:
    cmp qword ptr [rip + .Lsignal_mode], 3
    je .Lsignal_handler_entry
    cmp edi, 11
    jne .Lfailure
.Lsignal_handler_entry:
    lea rax, [rsp + 312]
    cmp rsi, rax
    jne .Lfailure
    lea rax, [rsp + 8]
    cmp rdx, rax
    jne .Lfailure
    cmp qword ptr [rdx], 6
    jne .Lfailure
    cmp qword ptr [rdx + 8], 0
    jne .Lfailure
    cmp qword ptr [rdx + 224], 0
    jne .Lfailure
    inc qword ptr [rip + .Lsignal_count]
    cmp qword ptr [rip + .Lsignal_mode], 2
    je .Lsignal_fault_handler
    cmp qword ptr [rdx + 16], 0
    jne .Lfailure
    cmp dword ptr [rdx + 24], 2
    jne .Lfailure
    cmp qword ptr [rdx + 32], 0
    jne .Lfailure
    cmp qword ptr [rip + .Lsignal_mode], 3
    je .Lsignal_generated_handler
    cmp qword ptr [rip + .Lsignal_mode], 0
    je .Lsignal_handler_return
    cmp dword ptr [rsi], 11
    jne .Lfailure
    cmp dword ptr [rsi + 4], 0
    jne .Lfailure
    cmp dword ptr [rsi + 8], -6
    jne .Lfailure
    cmp dword ptr [rsi + 16], 1
    jne .Lfailure
    cmp dword ptr [rsi + 20], 0
    jne .Lfailure
    cmp qword ptr [rdx + 192], 0
    jne .Lfailure
    cmp qword ptr [rdx + 200], 0
    jne .Lfailure
    cmp qword ptr [rdx + 208], 0
    jne .Lfailure
    cmp qword ptr [rdx + 216], 0
    jne .Lfailure
    cmp qword ptr [rdx + 296], 0
    jne .Lfailure
    mov qword ptr [rdx + 224], 1
    add rsp, 8
    mov eax, 15
    syscall
    sub rsp, 8
    cmp rax, -22
    jne .Lfailure
    mov qword ptr [rdx + 224], 0
    mov qword ptr [rdx + 144], 0x1234
    mov qword ptr [rdx + 296], 0x800
    mov edi, 99
    xor esi, esi
    lea rdx, [rip + .Lobserved_signal_mask]
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne .Lfailure
    cmp qword ptr [rip + .Lobserved_signal_mask], 0x200
    jne .Lfailure
    jmp .Lsignal_handler_return
.Lsignal_generated_handler:
    cmp rdi, qword ptr [rip + .Lexpected_signal]
    jne .Lfailure
    cmp dword ptr [rsi], edi
    jne .Lfailure
    cmp dword ptr [rsi + 4], 0
    jne .Lfailure
    mov rax, qword ptr [rip + .Lexpected_signal_code]
    cmp dword ptr [rsi + 8], eax
    jne .Lfailure
    cmp dword ptr [rsi + 16], 1
    jne .Lfailure
    cmp dword ptr [rsi + 20], 0
    jne .Lfailure
    cmp qword ptr [rdx + 192], 0
    jne .Lfailure
    cmp qword ptr [rdx + 200], 0
    jne .Lfailure
    cmp qword ptr [rdx + 216], 0
    jne .Lfailure
    cmp qword ptr [rip + .Lexpected_signal], 12
    jne .Lsignal_generated_first
    cmp qword ptr [rdx + 144], 0x5678
    jne .Lfailure
    jmp .Lsignal_generated_query_mask
.Lsignal_generated_first:
    cmp qword ptr [rip + .Lsignal_count], 5
    jne .Lsignal_generated_query_mask
    mov qword ptr [rdx + 144], 0x5678
.Lsignal_generated_query_mask:
    mov edi, 99
    xor esi, esi
    lea rdx, [rip + .Lobserved_signal_mask]
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne .Lfailure
    mov rax, qword ptr [rip + .Lexpected_signal_mask]
    cmp qword ptr [rip + .Lobserved_signal_mask], rax
    jne .Lfailure
    cmp qword ptr [rip + .Lexpected_signal], 10
    jne .Lsignal_handler_return
    mov qword ptr [rip + .Lexpected_signal], 12
    mov qword ptr [rip + .Lexpected_signal_code], -6
    mov qword ptr [rip + .Lexpected_signal_mask], 0x800
    jmp .Lsignal_handler_return
.Lsignal_fault_handler:
    cmp rsp, 0x402800
    jb .Lfailure
    cmp rsp, 0x403000
    jae .Lfailure
    cmp qword ptr [rdx + 16], 0x402800
    jne .Lfailure
    cmp dword ptr [rdx + 24], 0x80000000
    jne .Lfailure
    cmp qword ptr [rdx + 32], 2048
    jne .Lfailure
    cmp dword ptr [rsi], 11
    jne .Lfailure
    cmp dword ptr [rsi + 4], 0
    jne .Lfailure
    cmp dword ptr [rsi + 8], 1
    jne .Lfailure
    cmp qword ptr [rsi + 16], 0x40b000
    jne .Lfailure
    cmp qword ptr [rdx + 200], 14
    jne .Lfailure
    cmp qword ptr [rdx + 208], 0
    jne .Lfailure
    cmp qword ptr [rdx + 216], 0x40b000
    jne .Lfailure
    cmp qword ptr [rdx + 296], 0
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
    xor edi, edi
    mov esi, 4096
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40b000
    jne .Lfailure
.Lsignal_handler_return:
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
.Lsignal_mode:
    .zero 8
.Lobserved_signal_mask:
    .zero 8
.Lold_signal_action:
    .zero 32
.Lwait_info:
    .zero 128
.Lwait_timeout:
    .zero 16
.Lexpected_signal:
    .zero 8
.Lexpected_signal_code:
    .zero 8
.Lexpected_signal_mask:
    .zero 8
.Lpipe_descriptors:
    .zero 8
#endif

.section .note.GNU-stack,"",@progbits

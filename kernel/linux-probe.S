.intel_syntax noprefix
.section .text
.global ginux_linux_probe
#ifdef GINUX_LINKED
.type ginux_dependency_symbol,@function
#ifdef GINUX_GRAPH
.type ginux_auxiliary_symbol,@function
#endif
#endif
ginux_linux_probe:
#ifdef GINUX_LINKED
#ifdef GINUX_GRAPH
    movabs rax, 0x4e49414d54494e49
    cmp qword ptr [rip + ginux_main_init_state], rax
    jne failure
    mov rcx, qword ptr [rsp]
    cmp rcx, 8
    ja failure
    lea rax, [rsp + rcx*8 + 16]
    xor ecx, ecx
.Lentry_environment_scan:
    cmp rcx, 8
    ja failure
    cmp qword ptr [rax + rcx*8], 0
    je .Lentry_auxiliary_scan_start
    inc rcx
    jmp .Lentry_environment_scan
.Lentry_auxiliary_scan_start:
    lea rax, [rax + rcx*8 + 8]
    mov ecx, 32
.Lentry_auxiliary_scan:
    cmp qword ptr [rax], 26
    je .Lentry_hardware_capability_two
    cmp qword ptr [rax], 0
    je failure
    add rax, 16
    loop .Lentry_auxiliary_scan
    jmp failure
.Lentry_hardware_capability_two:
    cmp qword ptr [rax + 8], 1
    jne .Linitial_graph_entry
    cmp qword ptr [rax - 256], 3
    jne failure
    cmp qword ptr [rax - 248], 0x3ff040
    jne failure
    cmp qword ptr [rax - 240], 4
    jne failure
    cmp qword ptr [rax - 232], 56
    jne failure
    cmp qword ptr [rax - 224], 5
    jne failure
    cmp qword ptr [rax - 216], 9
    jne failure
    cmp qword ptr [rax - 208], 6
    jne failure
    cmp qword ptr [rax - 200], 4096
    jne failure
    cmp qword ptr [rax - 192], 7
    jne failure
    cmp qword ptr [rax - 184], 0x600000
    jne failure
    cmp qword ptr [rax - 176], 8
    jne failure
    cmp qword ptr [rax - 168], 0
    jne failure
    cmp qword ptr [rax - 160], 11
    jne failure
    cmp qword ptr [rax - 152], 0
    jne failure
    cmp qword ptr [rax - 144], 12
    jne failure
    cmp qword ptr [rax - 136], 0
    jne failure
    cmp qword ptr [rax - 128], 13
    jne failure
    cmp qword ptr [rax - 120], 0
    jne failure
    cmp qword ptr [rax - 112], 14
    jne failure
    cmp qword ptr [rax - 104], 0
    jne failure
    cmp qword ptr [rax - 96], 23
    jne failure
    cmp qword ptr [rax - 88], 0
    jne failure
    cmp qword ptr [rax - 80], 31
    jne failure
    mov rdx, qword ptr [rax - 72]
    cmp dword ptr [rdx], 0x696e692f
    jne failure
    cmp word ptr [rdx + 4], 0x74
    jne failure
    cmp qword ptr [rax - 64], 9
    jne failure
    cmp qword ptr [rax - 56], 0x400100
    jne failure
    cmp qword ptr [rax - 48], 17
    jne failure
    cmp qword ptr [rax - 40], 100
    jne failure
    cmp qword ptr [rax - 32], 15
    jne failure
    mov rdx, qword ptr [rax - 24]
    cmp dword ptr [rdx], 0x5f363878
    jne failure
    cmp word ptr [rdx + 4], 0x3436
    jne failure
    cmp byte ptr [rdx + 6], 0
    jne failure
    cmp qword ptr [rax - 16], 16
    jne failure
    cmp qword ptr [rax - 8], 0
    jne failure
    cmp qword ptr [rax + 16], 0
    jne failure
    cmp qword ptr [rax + 24], 0
    jne failure
    jmp .Ldynamic_exec_complete
.Linitial_graph_entry:
#else
    mov rax, qword ptr [rip + ginux_dependency_symbol@GOTPCREL]
    test rax, rax
    jz failure
    call ginux_dependency_symbol@PLT
    movabs rcx, 0x59434e45444e4947
    cmp rax, rcx
    jne failure

#endif

#ifndef GINUX_GRAPH
    mov edi, -100
    lea rsi, [rip + auxiliary_dependency_path]
    xor edx, edx
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure
    mov edi, 2
    lea rsi, [rsp - 32]
    mov edx, 4
    xor eax, eax
    syscall
    cmp rax, 4
    jne failure
    cmp dword ptr [rsp - 32], 0x464c457f
    jne failure
    mov eax, 3
    syscall
    test rax, rax
    jne failure
#endif
#endif
    cmp qword ptr [rsp], 1
    jne failure
    mov rax, qword ptr [rsp + 8]
    cmp byte ptr [rax], '/'
    jne failure
    mov rax, qword ptr [rsp + 24]
    cmp byte ptr [rax], 'G'
    jne failure
    mov rax, qword ptr [rsp + 112]
    test rax, rax
    jz static_relocation_state
    cmp rax, 0x600000
    jne failure
#ifndef GINUX_LINKED
    cmp qword ptr [0x406100], 0x400100
    jne failure
    cmp qword ptr [0x406108], 0x610100
    jne failure
    cmp qword ptr [0x406110], 0x610100
    jne failure
    call qword ptr [0x406110]
    movabs rcx, 0x59434e45444e4947
    cmp rax, rcx
    jne failure
#endif
    jmp valid_interpreter_base
static_relocation_state:
    cmp qword ptr [0x406100], 0
    jne failure
    cmp qword ptr [0x406108], 0
    jne failure
    cmp qword ptr [0x406110], 0
    jne failure
valid_interpreter_base:
    cmp qword ptr [rsp + 128], 0
    jne failure
    cmp qword ptr [rsp + 208], 0
    jne failure
    mov rax, qword ptr [rsp + 224]
    cmp byte ptr [rax], '/'
    jne failure
    cmp qword ptr [rsp + 240], 0x400100
    jne failure
    cmp qword ptr [rsp + 248], 17
    jne failure
    cmp qword ptr [rsp + 256], 100
    jne failure
    cmp qword ptr [rsp + 264], 15
    jne failure
    mov rax, qword ptr [rsp + 272]
    cmp dword ptr [rax], 0x5f363878
    jne failure
    cmp word ptr [rax + 4], 0x3436
    jne failure
    cmp byte ptr [rax + 6], 0
    jne failure
    cmp qword ptr [rsp + 280], 16
    jne failure
    cmp qword ptr [rsp + 288], 0
    jne failure
    cmp qword ptr [rsp + 296], 26
    jne failure
    cmp qword ptr [rsp + 304], 0
    jne failure
    cmp qword ptr [rsp + 312], 0
    jne failure
    cmp qword ptr [rsp + 320], 0
    jne failure

    mov edi, 1
    lea rsi, [rip + readable_payload]
    mov edx, 8
    mov eax, 1
    call syscall_expect_eight

    mov edi, 0x400000
    mov eax, 63
    call syscall_expect_efault

    movabs rax, 0x0a212145424f5250
    mov qword ptr [rsp - 16], rax
    mov edi, 1
    lea rsi, [rsp - 16]
    mov edx, 8
    mov eax, 1
    call syscall_expect_eight
    mov edi, 1
    mov esi, 0x409ff9
    mov edx, 8
    mov eax, 1
    call syscall_expect_efault

    xor edi, edi
    xor esi, esi
    xor edx, edx
    mov eax, 7
    call syscall_expect_zero
    mov edx, 1
    mov eax, 7
    call syscall_expect_enosys

    mov edi, 0x405c80
    mov esi, 5
    xor edx, edx
    mov eax, 7
    call syscall_expect_einval
    mov edi, 0x4061fc
    mov esi, 1
    mov eax, 7
    call syscall_expect_efault

    mov dword ptr [0x405c80], 0
    mov word ptr [0x405c84], 1
    mov word ptr [0x405c86], -1
    mov dword ptr [0x405c88], 1
    mov word ptr [0x405c8c], 4
    mov word ptr [0x405c8e], -1
    mov dword ptr [0x405c90], 99
    mov word ptr [0x405c94], 0
    mov word ptr [0x405c96], -1
    mov dword ptr [0x405c98], -1
    mov word ptr [0x405c9c], -1
    mov word ptr [0x405c9e], -1
    mov edi, 0x405c80
    mov esi, 4
    mov edx, -1
    mov eax, 7
    call syscall_expect_three
    cmp word ptr [0x405c86], 0x10
    jne failure
    cmp word ptr [0x405c8e], 4
    jne failure
    cmp word ptr [0x405c96], 0x20
    jne failure
    cmp word ptr [0x405c9e], 0
    jne failure

    mov dword ptr [0x405ca0], 1
    mov word ptr [0x405ca4], 0
    mov word ptr [0x405ca6], 0x7777
    mov edi, 0x405ca0
    mov esi, 1
    xor edx, edx
    mov eax, 7
    call syscall_expect_zero
    cmp word ptr [0x405ca6], 0
    jne failure
    mov word ptr [0x405ca6], 0x7777
    mov edx, 1
    mov eax, 7
    call syscall_expect_enosys
    cmp word ptr [0x405ca6], 0x7777
    jne failure

    xor edi, edi
    xor esi, esi
    mov edx, 0x406200
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 271
    call syscall_expect_efault

    mov qword ptr [0x405cc0], 0
    mov qword ptr [0x405cc8], 1000000000
    mov edx, 0x405cc0
    mov eax, 271
    call syscall_expect_einval

    mov qword ptr [0x405cc8], 0
    mov edi, 0x405ca0
    mov esi, 1
    mov eax, 271
    call syscall_expect_zero
    cmp word ptr [0x405ca6], 0
    jne failure

    mov qword ptr [0x405cc8], 1
    mov word ptr [0x405ca6], 0x7777
    mov eax, 271
    call syscall_expect_enosys
    cmp word ptr [0x405ca6], 0x7777
    jne failure

    mov qword ptr [0x405cd0], 0
    mov edi, 0x405c80
    mov esi, 4
    xor edx, edx
    mov r10d, 0x405cd0
    mov r8d, 7
    mov eax, 271
    call syscall_expect_einval
    mov r8d, 8
    mov eax, 271
    call syscall_expect_three

    xor edi, edi
    mov eax, 12
    syscall
    cmp rax, 0x407000
    jne failure
    mov edi, 0x407080
    mov eax, 12
    syscall
    cmp rax, 0x407080
    jne failure
    movabs rax, 0x0a212145424f5250
    mov qword ptr [0x407000], rax
    mov edi, 1
    mov esi, 0x407000
    mov edx, 8
    mov eax, 1
    call syscall_expect_eight
    mov edi, 1
    mov esi, 0x407079
    mov edx, 8
    mov eax, 1
    call syscall_expect_efault
    movabs rax, 0x4845415042524b21
    mov qword ptr [0x407000], rax
    mov edi, 0x408001
    mov eax, 12
    syscall
    cmp rax, 0x407080
    jne failure
    mov edi, 0x407040
    mov eax, 12
    syscall
    cmp rax, 0x407040
    jne failure

    xor edi, edi
    mov esi, 4096
    mov edx, 7
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    call syscall_expect_einval

    xor edi, edi
    mov esi, 4096
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40b000
    jne failure
    cmp qword ptr [0x40b000], 0
    jne failure

    mov edi, 0x40b000
    mov eax, 63
    call syscall_expect_zero
    cmp dword ptr [0x40b000], 0x756e6947
    jne failure

    xor edi, edi
    mov esi, 4096
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40c000
    jne failure
    cmp qword ptr [0x40c000], 0
    jne failure
    mov byte ptr [0x405949], 0
    mov edi, 0x40c000
    mov esi, 4096
    mov edx, 0x405949
    mov eax, 27
    call syscall_expect_zero
    cmp byte ptr [0x405949], 1
    jne failure
    mov eax, 11
    call syscall_expect_zero
    mov edi, 1
    mov esi, 0x40c000
    mov edx, 8
    mov eax, 1
    call syscall_expect_efault

    mov edi, 0x40b000
    mov esi, 4095
    mov eax, 11
    call syscall_expect_einval
    mov esi, 4096
    mov eax, 11
    call syscall_expect_zero

    mov edi, 1
    mov esi, 0x40b000
    mov edx, 8
    mov eax, 1
    call syscall_expect_efault

    xor edi, edi
    mov esi, 4096
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40b000
    jne failure
    cmp qword ptr [0x40b000], 0
    jne failure
    mov qword ptr [0x40b000], 2

    mov edi, 0x40b000
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    call syscall_expect_zero
    mov eax, 10
    call syscall_expect_einval

    mov edi, 0x40b000
    mov eax, 63
    call syscall_expect_efault

    mov edi, 2
    mov esi, 0x40b000
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    call syscall_expect_zero
    mov qword ptr [0x405940], 0
    mov esi, 0x405940
    mov eax, 14
    call syscall_expect_zero

    mov edi, 0x40b000
    mov esi, 4096
    mov edx, 3
    mov eax, 28
    call syscall_expect_einval
    cmp qword ptr [0x40b000], 2
    jne failure
    mov edx, 4
    mov eax, 28
    call syscall_expect_zero
    cmp qword ptr [0x40b000], 0
    jne failure

    mov edi, 0x40b000
    mov esi, 4096
    mov eax, 11
    call syscall_expect_zero

    xor edi, edi
    mov esi, 4096
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40b000
    jne failure
    cmp qword ptr [0x40b000], 0
    jne failure

    mov edi, 0x40b001
    mov esi, 4096
    mov edx, 0x405948
    mov eax, 27
    call syscall_expect_einval
    mov edi, 0x40b000
    mov edx, 0x406200
    mov eax, 27
    call syscall_expect_efault
    mov byte ptr [0x405948], 0
    mov edx, 0x405948
    mov eax, 27
    call syscall_expect_zero
    cmp byte ptr [0x405948], 1
    jne failure

    mov edi, 0x40b000
    mov eax, 63
    call syscall_expect_zero
    mov edi, 0x40b000
    mov esi, 4096
    mov eax, 11
    call syscall_expect_zero
    mov edx, 0x405948
    mov eax, 27
    syscall
    cmp rax, -12
    jne failure
    mov eax, 11
    call syscall_expect_zero

    xor edi, edi
    mov esi, 8192
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40b000
    jne failure
    cmp qword ptr [0x40b000], 0
    jne failure
    cmp qword ptr [0x40c000], 0
    jne failure

    mov qword ptr [0x40c000], -1
    mov edi, 0x40bf80
    mov eax, 63
    call syscall_expect_zero
    cmp dword ptr [0x40bf80], 0x756e6947
    jne failure
    cmp word ptr [0x40c000], 0
    jne failure

    mov byte ptr [0x40594a], 0
    mov byte ptr [0x40594b], 0
    mov edi, 0x40b000
    mov esi, 8192
    mov edx, 0x40594a
    mov eax, 27
    call syscall_expect_zero
    cmp byte ptr [0x40594a], 1
    jne failure
    cmp byte ptr [0x40594b], 1
    jne failure

    mov qword ptr [0x40b000], 1
    mov qword ptr [0x40c000], 2
    mov edx, 4
    mov eax, 28
    call syscall_expect_zero
    cmp qword ptr [0x40b000], 0
    jne failure
    cmp qword ptr [0x40c000], 0
    jne failure

    mov edi, 0x40b000
    mov esi, 8192
    mov edx, 1
    mov eax, 10
    call syscall_expect_zero
    mov eax, 10
    call syscall_expect_einval
    mov edi, 0x40bf80
    mov eax, 63
    call syscall_expect_efault
    mov edi, 0x40b000

    mov esi, 4096
    mov eax, 11
    call syscall_expect_zero
    mov edi, 1
    mov esi, 0x40b000
    mov edx, 8
    mov eax, 1
    call syscall_expect_efault
    mov byte ptr [0x40594a], 0
    mov edi, 0x40c000
    mov esi, 4096
    mov edx, 0x40594a
    mov eax, 27
    call syscall_expect_zero
    cmp byte ptr [0x40594a], 1
    jne failure
    cmp qword ptr [0x40c000], 0
    jne failure
    mov edi, 0x40c000
    mov esi, 4096
    mov eax, 11
    call syscall_expect_zero

    xor edi, edi
    mov esi, 8192
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40b000
    jne failure
    mov qword ptr [0x40b000], 3
    mov qword ptr [0x40c000], 4
    mov edi, 0x40c000
    mov esi, 4096
    mov eax, 11
    call syscall_expect_zero
    cmp qword ptr [0x40b000], 3
    jne failure
    mov edi, 1
    mov esi, 0x40c000
    mov edx, 8
    mov eax, 1
    call syscall_expect_efault
    mov byte ptr [0x40594a], 0
    mov edi, 0x40b000
    mov esi, 4096
    mov edx, 0x40594a
    mov eax, 27
    call syscall_expect_zero
    cmp byte ptr [0x40594a], 1
    jne failure
    mov edi, 0x40b000
    mov esi, 4096
    mov eax, 11
    call syscall_expect_zero

    xor edi, edi
    mov esi, 8192
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40b000
    jne failure
    mov qword ptr [0x40b000], 5
    mov qword ptr [0x40c000], 6
    mov edi, 0x40b000
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    call syscall_expect_zero
    mov eax, 10
    call syscall_expect_einval
    mov edi, 0x40b000
    mov eax, 63
    call syscall_expect_efault
    cmp qword ptr [0x40b000], 5
    jne failure
    mov edi, 0x40c000
    mov eax, 63
    call syscall_expect_zero
    cmp dword ptr [0x40c000], 0x756e6947
    jne failure
    mov edi, 0x40c000
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    call syscall_expect_zero
    mov edi, 0x40c000
    mov eax, 63
    call syscall_expect_efault
    mov edi, 0x40b000
    mov esi, 4096
    mov eax, 11
    call syscall_expect_zero
    mov edi, 0x40c000
    mov eax, 11
    call syscall_expect_zero

    xor edi, edi
    mov esi, 8192
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40b000
    jne failure
    mov qword ptr [0x40b000], 7
    mov qword ptr [0x40c000], 8
    mov edi, 0x40c000
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    call syscall_expect_zero
    mov edi, 0x40b000
    mov eax, 63
    call syscall_expect_zero
    cmp dword ptr [0x40b000], 0x756e6947
    jne failure
    mov edi, 0x40c000
    mov eax, 63
    call syscall_expect_efault
    cmp qword ptr [0x40c000], 8
    jne failure
    mov edi, 0x40b000
    mov esi, 4096
    mov eax, 11
    call syscall_expect_zero
    mov edi, 0x40c000
    mov eax, 11
    call syscall_expect_zero

    xor edi, edi
    mov esi, 4096
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40b000
    jne failure
    xor edi, edi
    mov eax, 9
    syscall
    cmp rax, 0x40c000
    jne failure
    mov qword ptr [0x40b000], 9
    mov qword ptr [0x40c000], 10
    mov byte ptr [0x40594a], 0
    mov byte ptr [0x40594b], 0
    mov edi, 0x40b000
    mov esi, 8192
    mov edx, 0x40594a
    mov eax, 27
    call syscall_expect_zero
    cmp byte ptr [0x40594a], 1
    jne failure
    cmp byte ptr [0x40594b], 1
    jne failure
    mov edx, 4
    mov eax, 28
    call syscall_expect_zero
    cmp qword ptr [0x40b000], 0
    jne failure
    cmp qword ptr [0x40c000], 0
    jne failure
    mov edx, 1
    mov eax, 10
    call syscall_expect_zero
    mov edi, 0x40b000
    mov eax, 63
    call syscall_expect_efault
    mov edi, 0x40c000
    mov eax, 63
    call syscall_expect_efault
    mov edi, 0x40b000
    mov esi, 8192
    mov eax, 11
    call syscall_expect_zero
    mov eax, 11
    call syscall_expect_zero
    mov edi, 1
    mov esi, 0x40b000
    mov edx, 8
    mov eax, 1
    call syscall_expect_efault
    mov esi, 0x40c000
    mov eax, 1
    call syscall_expect_efault

#if defined(GINUX_LINKED) && defined(GINUX_GRAPH)
    xor edi, edi
    mov esi, 0x411000
    mov edx, 0x405194
    lea r8d, [edx + 4]
    mov r9d, 0x40efc0
    mov eax, 56
    call syscall_expect_einval
    mov r14d, 2
linux_clone_repeat:
    mov edi, 0x13d0f00
    mov esi, 0x411000
    mov edx, 0x405194
    lea r8d, [edx + 4]
    mov r9d, 0x40efc0
    mov eax, 56
    syscall
    test rax, rax
    jz child_thread_probe
    cmp eax, dword ptr [rdx]
    jne failure
    cmp eax, dword ptr [r8]
    jne failure
    mov ebx, eax
    mov ebp, edx
    mov eax, 24
    call syscall_expect_zero
    mov edi, 0x13d0f00
    mov esi, 0x411000
    mov edx, ebp
    lea r8d, [edx + 4]
    mov r9d, 0x40efc0
    mov eax, 56
    call syscall_expect_eagain
    mov eax, 24
    call syscall_expect_zero
    mov rdi, rbp
    xor esi, esi
    inc esi
    mov edx, esi
    mov eax, 202
    call syscall_expect_one
    mov rdi, r8
    xor esi, esi
    mov edx, ebx
    mov eax, 202
    call syscall_expect_zero
    cmp dword ptr [r8], 0
    jne failure
    cmp dword ptr [rbp], ebx
    jne failure
    dec r14d
    jnz linux_clone_repeat
#else
    mov eax, 24
    call syscall_expect_zero
#endif

    xor edi, edi
    mov eax, 145
    call syscall_expect_zero

    mov dword ptr [0x405060], 0x12345678
    mov esi, 0x405060
    mov eax, 143
    call syscall_expect_zero
    cmp dword ptr [0x405060], 0
    jne failure

    xor edi, edi
    mov eax, 146
    call syscall_expect_zero
    mov eax, 147
    call syscall_expect_zero

    mov dword ptr [0x405060], 0
    mov esi, 0x405060
    mov eax, 142
    call syscall_expect_zero
    xor esi, esi
    mov edx, 0x405060
    mov eax, 144
    call syscall_expect_zero

    xor edi, edi
    xor esi, esi
    mov eax, 140
    syscall
    cmp rax, 20
    jne failure
    xor edi, edi
    xor esi, esi
    xor edx, edx
    mov eax, 141
    call syscall_expect_zero

    mov edi, 2
    mov esi, 0x405000
    mov edx, 8
    mov eax, 1
    call syscall_expect_ebadf

    mov edi, 1
    mov esi, 0x405000
    mov edx, 257
    mov eax, 1
    call syscall_expect_einval

    mov qword ptr [0x4050c0], 0x40102
    mov edi, 99
    xor esi, esi
    mov edx, 0x4050e0
    mov r10d, 7
    mov eax, 14
    call syscall_expect_einval

    mov edi, 99
    xor esi, esi
    mov edx, 0x4050e0
    mov r10d, 8
    mov eax, 14
    call syscall_expect_zero
    cmp qword ptr [0x4050e0], 0
    jne failure

    xor edi, edi
    mov esi, 0x4050c0
    mov edx, 0x4050e0
    mov r10d, 8
    mov eax, 14
    call syscall_expect_zero
    cmp qword ptr [0x4050e0], 0
    jne failure

    mov edi, 99
    xor esi, esi
    mov edx, 0x4050e0
    mov r10d, 8
    mov eax, 14
    call syscall_expect_zero
    cmp qword ptr [0x4050e0], 2
    jne failure

    mov qword ptr [0x4050c0], 8
    xor edi, edi
    mov esi, 0x4050c0
    mov edx, 0x406200
    mov r10d, 8
    mov eax, 14
    call syscall_expect_efault

    mov edi, 99
    xor esi, esi
    mov edx, 0x4050e0
    mov r10d, 8
    mov eax, 14
    call syscall_expect_zero
    cmp qword ptr [0x4050e0], 2
    jne failure

    mov qword ptr [0x4050c0], 2
    mov edi, 1
    mov esi, 0x4050c0
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    call syscall_expect_zero

    mov edi, 99
    xor esi, esi
    mov edx, 0x4050e0
    mov r10d, 8
    mov eax, 14
    call syscall_expect_zero
    cmp qword ptr [0x4050e0], 0
    jne failure

    xor edi, edi
    mov esi, 0x406200
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    call syscall_expect_efault

    mov edi, 10
    xor esi, esi
    xor edx, edx
    mov r10d, 7
    mov eax, 13
    call syscall_expect_einval

    xor edi, edi
    xor esi, esi
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    call syscall_expect_einval

    mov qword ptr [0x405100], 1
    mov qword ptr [0x405108], 0x1234
    mov qword ptr [0x405110], 0x400100
    mov qword ptr [0x405118], 0x40102
    mov edi, 9
    mov esi, 0x405100
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    call syscall_expect_einval

    mov edi, 10
    mov esi, 0x406200
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    call syscall_expect_efault

    mov edi, 10
    mov esi, 0x405100
    mov edx, 0x405120
    mov r10d, 8
    mov eax, 13
    call syscall_expect_zero

    cmp qword ptr [0x405120], 0
    jne failure
    cmp qword ptr [0x405138], 0
    jne failure

    mov edi, 10
    xor esi, esi
    mov edx, 0x405120
    mov r10d, 8
    mov eax, 13
    call syscall_expect_zero
    cmp qword ptr [0x405120], 1
    jne failure
    cmp qword ptr [0x405128], 0x1234
    jne failure
    cmp qword ptr [0x405130], 0x400100
    jne failure
    cmp qword ptr [0x405138], 2
    jne failure

    mov qword ptr [0x405100], 0
    mov qword ptr [0x405108], 0
    mov qword ptr [0x405110], 0
    mov qword ptr [0x405118], 0
    mov edi, 10
    mov esi, 0x405100
    mov edx, 0x406200
    mov r10d, 8
    mov eax, 13
    call syscall_expect_efault

    mov edi, 10
    xor esi, esi
    mov edx, 0x405120
    mov r10d, 8
    mov eax, 13
    call syscall_expect_zero
    cmp qword ptr [0x405120], 1
    jne failure

    mov edi, 10
    mov esi, 0x405100
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    call syscall_expect_zero

    xor edi, edi
    mov esi, 0x405158
    mov eax, 131
    call syscall_expect_zero
    cmp qword ptr [0x405158], 0
    jne failure
    cmp dword ptr [0x405160], 2
    jne failure
    cmp qword ptr [0x405168], 0
    jne failure

    xor edi, edi
    mov esi, 0x406200
    mov eax, 131
    call syscall_expect_efault

    mov qword ptr [0x405140], 0x405800
    mov qword ptr [0x405148], 0
    mov qword ptr [0x405150], 2048
    mov edi, 0x405140
    mov esi, 0x405158
    mov eax, 131
    call syscall_expect_zero
    cmp dword ptr [0x405160], 2
    jne failure

    xor edi, edi
    mov esi, 0x405158
    mov eax, 131
    call syscall_expect_zero
    cmp qword ptr [0x405158], 0x405800
    jne failure
    cmp dword ptr [0x405160], 0
    jne failure
    cmp qword ptr [0x405168], 2048
    jne failure

    mov qword ptr [0x405140], 0x406000
    mov qword ptr [0x405148], 0
    mov qword ptr [0x405150], 2048
    mov edi, 0x405140
    xor esi, esi
    mov eax, 131
    call syscall_expect_efault

    mov qword ptr [0x405140], 0x405800
    mov qword ptr [0x405148], 0
    mov qword ptr [0x405150], 1024
    mov edi, 0x405140
    xor esi, esi
    mov eax, 131
    syscall
    cmp rax, -12
    jne failure

    mov qword ptr [0x405140], 0
    mov qword ptr [0x405148], 2
    mov qword ptr [0x405150], 0
    mov edi, 0x405140
    xor esi, esi
    mov eax, 131
    call syscall_expect_zero

    mov edi, 1
    mov esi, 0x405000
    mov edx, 8
    mov eax, 1
    call syscall_expect_eight

    mov edi, 0x1002
    mov esi, 0x405000
    mov eax, 158
    call syscall_expect_zero
    mov rbx, qword ptr fs:[0]
    movabs rax, 0x0a212145424f5250
    cmp rbx, rax
    jne failure

    mov edi, 0x1003
    mov esi, 0x406200
    mov eax, 158
    call syscall_expect_efault

    xor edi, edi
    xor esi, esi
    mov eax, 158
    call syscall_expect_einval

    mov edi, 0x1003
    mov esi, 0x405008
    mov eax, 158
    call syscall_expect_zero
    cmp qword ptr [0x405008], 0x405000
    jne failure

    mov edi, 0x405020
    mov esi, 1
    mov eax, 79
    syscall
    cmp rax, -34
    jne failure
    mov edi, 0x406200
    mov esi, 2
    mov eax, 79
    call syscall_expect_efault
    mov edi, 0x405020
    mov esi, 2
    mov eax, 79
    call syscall_expect_two
    cmp word ptr [0x405020], 0x002f
    jne failure

    lea rdi, [rip + proc_self_exe]
    mov esi, 0x405040
    xor edx, edx
    mov eax, 89
    call syscall_expect_einval
    mov edi, 0x40a000
    mov esi, 0x405040
    mov edx, 5
    mov eax, 89
    call syscall_expect_efault
    lea rdi, [rip + proc_self_exe]
    mov esi, 0x406200
    mov edx, 5
    mov eax, 89
    call syscall_expect_efault
    lea rdi, [rip + proc_self_exe]
    mov esi, 0x405040
    mov edx, 5
    mov eax, 89
    call syscall_expect_five
    cmp dword ptr [0x405040], 0x696e692f
    jne failure
    cmp byte ptr [0x405044], 't'
    jne failure

    mov edi, -100
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x405040
    mov r10d, 5
    mov eax, 267
    call syscall_expect_five
    cmp dword ptr [0x405040], 0x696e692f
    jne failure
    cmp byte ptr [0x405044], 't'
    jne failure

    mov edi, -99
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x405040
    mov r10d, 5
    mov eax, 267
    call syscall_expect_ebadf

    mov edi, -100
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x405040
    xor r10d, r10d
    mov eax, 267
    call syscall_expect_einval

    mov edi, -100
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x406200
    mov r10d, 5
    mov eax, 267
    call syscall_expect_efault

    mov edi, 0x3f
    mov eax, 95
    syscall
    cmp rax, 0x12
    jne failure
    mov edi, 0x29c
    mov eax, 95
    syscall
    cmp rax, 0x3f
    jne failure
    mov edi, 0x12
    mov eax, 95
    syscall
    cmp rax, 0x9c
    jne failure

    mov edi, 0x406200
    mov eax, 63
    call syscall_expect_efault

    mov edi, 0x405000
    mov eax, 63
    call syscall_expect_zero
    cmp dword ptr [rdi], 0x756e6947
    jne failure

    mov edi, 2
    mov esi, 0x405200
    mov eax, 5
    call syscall_expect_ebadf

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4061c0], rax
    mov edi, 1
    mov esi, 0x4061c0
    mov eax, 5
    call syscall_expect_efault
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4061c0], rax
    jne failure

    mov edi, 1
    mov esi, 0x405200
    mov eax, 5
    call syscall_expect_zero
    cmp qword ptr [0x405210], 1
    jne failure
    cmp dword ptr [0x405218], 0x2180
    jne failure
    cmp qword ptr [0x405238], 4096
    jne failure

    mov edi, 2
    mov esi, 0x5413
    mov edx, 0x405280
    mov eax, 16
    call syscall_expect_ebadf

    mov edi, 1
    mov esi, 0x5402
    mov edx, 0x405280
    mov eax, 16
    syscall
    cmp rax, -25
    jne failure

    mov edi, 1
    mov esi, 0x5413
    mov edx, 0x4061fc
    mov eax, 16
    call syscall_expect_efault

    mov edi, 1
    mov esi, 0x5413
    mov edx, 0x405280
    mov eax, 16
    call syscall_expect_zero
    cmp word ptr [0x405280], 24
    jne failure
    cmp word ptr [0x405282], 80
    jne failure
    cmp dword ptr [0x405284], 0
    jne failure

    mov edi, 2
    movabs rsi, 0x1122334455667788
    mov edx, 2
    mov eax, 8
    call syscall_expect_ebadf

    mov edi, 1
    movabs rsi, 0x1122334455667788
    mov edx, 99
    mov eax, 8
    syscall
    cmp rax, -29
    jne failure

    lea rax, [rip + readable_payload]
    mov qword ptr [0x405300], rax
    mov qword ptr [0x405308], 4
    mov qword ptr [0x405310], 0x406200
    mov qword ptr [0x405318], 4
    mov edi, 1
    mov esi, 0x405300
    mov edx, 2
    mov eax, 20
    call syscall_expect_efault

    lea rax, [rip + readable_payload + 4]
    mov qword ptr [0x405310], rax
    mov edi, 2
    mov esi, 0x405300
    mov edx, 2
    mov eax, 20
    call syscall_expect_ebadf

    mov edi, 1
    mov esi, 0x405300
    mov edx, 5
    mov eax, 20
    call syscall_expect_einval

    mov edi, 1
    mov esi, 0x405300
    mov edx, 2
    mov eax, 20
    call syscall_expect_eight

    mov edi, 99
    mov esi, 0x405080
    mov eax, 97
    call syscall_expect_einval

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4061f8], rax
    mov edi, 3
    mov esi, 0x4061f8
    mov eax, 97
    call syscall_expect_efault
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4061f8], rax
    jne failure

    mov edi, 3
    mov esi, 0x405080
    mov eax, 97
    call syscall_expect_zero
    cmp qword ptr [0x405080], 0x1000
    jne failure
    cmp qword ptr [0x405088], 0x1000
    jne failure

    mov edi, 2
    mov esi, 7
    xor edx, edx
    mov r10d, 0x405090
    mov eax, 302
    syscall
    cmp rax, -3
    jne failure

    xor edi, edi
    mov esi, 99
    xor edx, edx
    mov r10d, 0x405090
    mov eax, 302
    call syscall_expect_einval

    xor edi, edi
    mov esi, 7
    mov edx, 0x405080
    xor r10d, r10d
    mov eax, 302
    syscall
    cmp rax, -1
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4061f8], rax
    xor edi, edi
    mov esi, 7
    xor edx, edx
    mov r10d, 0x4061f8
    mov eax, 302
    call syscall_expect_efault
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4061f8], rax
    jne failure

    xor edi, edi
    mov esi, 7
    xor edx, edx
    mov r10d, 0x405090
    mov eax, 302
    call syscall_expect_zero
    cmp qword ptr [0x405090], 6
    jne failure
    cmp qword ptr [0x405098], 6
    jne failure

    mov edi, 2
    mov esi, 8
    mov edx, 0x4050a0
    mov eax, 204
    syscall
    cmp rax, -3
    jne failure

    xor edi, edi
    mov esi, 7
    mov edx, 0x4050a0
    mov eax, 204
    call syscall_expect_einval

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4061fc], rax
    xor edi, edi
    mov esi, 8
    mov edx, 0x4061fc
    mov eax, 204
    call syscall_expect_efault
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4061fc], rax
    jne failure

    xor edi, edi
    mov esi, 8
    mov edx, 0x4050a0
    mov eax, 204
    call syscall_expect_eight
    cmp qword ptr [0x4050a0], 1
    jne failure

    mov eax, 203
    call syscall_expect_zero
    mov qword ptr [0x4050a0], 0
    mov eax, 203
    call syscall_expect_einval
    mov edx, 0x406200
    mov eax, 203
    call syscall_expect_efault
    mov qword ptr [0x4050a0], 1

    mov dword ptr [0x4050a8], 0x12345678
    mov edi, 0x4050a8
    mov esi, 0x406200
    mov edx, 0xdeadbeef
    mov eax, 309
    call syscall_expect_efault
    cmp dword ptr [0x4050a8], 0x12345678
    jne failure

    mov edi, 0x4050a8
    mov esi, 0x4050ac
    mov edx, 0xdeadbeef
    mov eax, 309
    call syscall_expect_zero
    cmp dword ptr [0x4050a8], 0
    jne failure
    cmp dword ptr [0x4050ac], 0
    jne failure

    mov edi, -100
    mov esi, 0x406200
    mov edx, 0x80002
    mov eax, 257
    call syscall_expect_efault

    movabs rax, 0x6c756e2f7665642f
    mov qword ptr [0x405340], rax
    mov word ptr [0x405348], 0x006c
    mov edi, -100
    mov esi, 0x405340
    mov edx, 0x80002
    mov eax, 257
    call syscall_expect_two

    mov edi, 2
    mov esi, 1
    mov eax, 72
    call syscall_expect_one
    mov esi, 3
    mov eax, 72
    call syscall_expect_two

    xor edi, edi
    mov eax, 291
    call syscall_expect_three

    mov edi, 3
    mov eax, 3
    call syscall_expect_zero

    mov edi, 7
    mov eax, 284
    call syscall_expect_three
    mov edi, 3
    mov esi, 0x405d90
    mov edx, 8
    xor eax, eax
    call syscall_expect_eight
    cmp qword ptr [0x405d90], 7
    jne failure
    mov eax, 3
    call syscall_expect_zero

    mov edi, 2
    mov esi, 0x801
    mov eax, 290
    call syscall_expect_three
    mov edi, 3
    mov esi, 0x405d90
    mov edx, 8
    xor eax, eax
    call syscall_expect_eight
    cmp qword ptr [0x405d90], 1
    jne failure
    xor eax, eax
    call syscall_expect_eight
    cmp qword ptr [0x405d90], 1
    jne failure
    xor eax, eax
    call syscall_expect_eagain
    mov eax, 3
    call syscall_expect_zero

    mov edi, 2
    mov esi, 0x406200
    mov edx, 8
    xor eax, eax
    call syscall_expect_zero
    mov eax, 1
    call syscall_expect_eight

    mov eax, 8
    call syscall_expect_zero
    mov esi, 0x405350
    mov eax, 5
    call syscall_expect_zero
    cmp dword ptr [0x405368], 0x2180
    jne failure

    mov edi, -100
    mov esi, 0x405340
    mov edx, 2
    mov eax, 257
    call syscall_expect_three
    mov eax, 257
    syscall
    cmp rax, 4
    jne failure
    mov eax, 257
    call syscall_expect_five
    mov eax, 257
    syscall
    cmp rax, -24
    jne failure
    mov edi, 3
    mov eax, 3
    call syscall_expect_zero
    mov edi, 4
    mov eax, 3
    call syscall_expect_zero
    mov edi, 5
    mov eax, 3
    call syscall_expect_zero

    mov edi, -100
    mov esi, 0x406200
    mov edx, 0x405350
    xor r10d, r10d
    mov eax, 262
    call syscall_expect_efault
    mov esi, 0x405340
    mov edx, 0x406200
    mov eax, 262
    call syscall_expect_efault
    mov edx, 0x405350
    mov r10d, 1
    mov eax, 262
    call syscall_expect_einval
    xor r10d, r10d
    mov eax, 262
    call syscall_expect_zero
    cmp dword ptr [0x405368], 0x2180
    jne failure

    mov edi, -100
    mov esi, 0x405340
    mov edx, 6
    mov eax, 269
    call syscall_expect_zero
    mov edx, 1
    mov eax, 269
    syscall
    cmp rax, -13
    jne failure
    mov esi, 0x406200
    xor edx, edx
    mov eax, 269
    call syscall_expect_efault
    mov esi, 0x405340
    mov edx, 8
    mov eax, 269
    call syscall_expect_einval

    xor edx, edx
    mov r10d, 1
    mov eax, 439
    call syscall_expect_einval
    xor r10d, r10d
    mov eax, 439
    call syscall_expect_zero

    mov edi, 2
    mov eax, 3
    call syscall_expect_zero
    mov eax, 3
    call syscall_expect_ebadf

    mov edi, 1
    mov eax, 291
    call syscall_expect_einval

    mov edi, 0x80000
    mov eax, 291
    call syscall_expect_two

    mov edi, 2
    mov esi, 1
    mov eax, 72
    call syscall_expect_one

    xor edi, edi
    mov eax, 291
    syscall
    cmp rax, -24
    jne failure

    xor edi, edi
    mov esi, 2
    mov eax, 290
    call syscall_expect_einval

    mov esi, 0x80800
    mov eax, 290
    call syscall_expect_three

    mov edi, 3
    mov esi, 1
    mov eax, 72
    call syscall_expect_one
    mov esi, 3
    mov eax, 72
    syscall
    cmp rax, 0x802
    jne failure
    mov esi, 4
    mov edx, 2
    mov eax, 72
    call syscall_expect_zero
    mov esi, 0x405d90
    mov edx, 8
    xor eax, eax
    call syscall_expect_enosys
    mov esi, 4
    mov edx, 0x802
    mov eax, 72
    call syscall_expect_zero
    mov edi, 2
    mov esi, 0x405350
    mov eax, 5
    call syscall_expect_zero
    cmp dword ptr [0x405368], 0x180
    jne failure
    mov edi, 3
    mov eax, 5
    call syscall_expect_zero
    cmp dword ptr [0x405368], 0x180
    jne failure
    mov eax, 8
    syscall
    cmp rax, -29
    jne failure

    mov dword ptr [0x405d00], 1
    movabs rax, 0xaabbccddeeff0011
    mov qword ptr [0x405d04], rax
    mov edi, 2
    mov esi, 1
    mov edx, 3
    mov r10d, 0x405d00
    mov eax, 233
    call syscall_expect_zero

    mov esi, 0x405d20
    mov edx, 1
    xor r10d, r10d
    mov eax, 232
    call syscall_expect_zero

    mov qword ptr [0x405d80], 5
    mov edi, 3
    mov esi, 0x405d80
    mov edx, 8
    mov eax, 1
    call syscall_expect_eight

    mov edi, 2
    mov esi, 0x405d20
    mov edx, 1
    xor r10d, r10d
    mov eax, 232
    call syscall_expect_one
    cmp dword ptr [0x405d20], 1
    jne failure
    movabs rax, 0xaabbccddeeff0011
    cmp qword ptr [0x405d24], rax
    jne failure

    mov edi, 3
    mov esi, 0x4061fc
    mov edx, 8
    xor eax, eax
    call syscall_expect_efault

    mov esi, 0x405d90
    xor eax, eax
    call syscall_expect_eight
    cmp qword ptr [0x405d90], 5
    jne failure

    mov edi, 2
    mov esi, 0x405d20
    mov edx, 1
    xor r10d, r10d
    mov eax, 232
    call syscall_expect_zero

    mov edi, 3
    mov esi, 0x405d90
    mov edx, 8
    xor eax, eax
    call syscall_expect_eagain

    mov qword ptr [0x405d80], -1
    mov esi, 0x405d80
    mov edx, 7
    mov eax, 1
    call syscall_expect_einval
    mov edx, 8
    mov eax, 1
    call syscall_expect_einval

    mov qword ptr [0x405d80], -2
    mov eax, 1
    call syscall_expect_eight

    mov dword ptr [0x405da0], 3
    mov word ptr [0x405da4], 4
    mov word ptr [0x405da6], -1
    mov edi, 0x405da0
    mov esi, 1
    xor edx, edx
    mov eax, 7
    call syscall_expect_zero
    cmp word ptr [0x405da6], 0
    jne failure

    mov qword ptr [0x405d80], 1
    mov edi, 3
    mov esi, 0x405d80
    mov edx, 8
    mov eax, 1
    call syscall_expect_eagain

    mov esi, 0x405d90
    xor eax, eax
    call syscall_expect_eight
    cmp qword ptr [0x405d90], -2
    jne failure

    mov edi, 2
    mov esi, 2
    mov edx, 3
    xor r10d, r10d
    mov eax, 233
    call syscall_expect_zero

    mov edi, 3
    mov eax, 3
    call syscall_expect_zero

    mov edi, 2
    mov esi, 1
    mov edx, 2
    mov r10d, 0x405d00
    mov eax, 233
    call syscall_expect_einval

    mov edx, 99
    mov eax, 233
    call syscall_expect_ebadf

    mov esi, 99
    mov edx, 1
    mov eax, 233
    call syscall_expect_einval

    mov dword ptr [0x405d00], 0x80000000
    mov esi, 1
    mov eax, 233
    call syscall_expect_einval

    mov dword ptr [0x405d00], 4
    movabs rax, 0x1122334455667788
    mov qword ptr [0x405d04], rax
    mov edi, 2
    mov esi, 1
    mov edx, 1
    mov r10d, 0x4061fc
    mov eax, 233
    call syscall_expect_efault

    mov r10d, 0x405d00
    mov eax, 233
    call syscall_expect_zero
    mov eax, 233
    syscall
    cmp rax, -17
    jne failure

    mov edi, 2
    mov esi, 0x405d20
    xor edx, edx
    xor r10d, r10d
    mov eax, 232
    call syscall_expect_einval

    mov qword ptr [0x405d40], 0
    mov edi, 2
    mov esi, 0x405d20
    mov edx, 1
    mov r10d, -1
    mov r8d, 0x405d40
    mov r9d, 7
    mov eax, 281
    call syscall_expect_einval

    mov r8d, 0x406200
    mov r9d, 8
    mov eax, 281
    call syscall_expect_efault

    mov r8d, 0x405d40
    mov eax, 281
    call syscall_expect_one
    cmp dword ptr [0x405d20], 4
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x405d24], rax
    jne failure

    mov qword ptr [0x405d60], 0
    mov qword ptr [0x405d68], 1000000000
    mov edi, 2
    mov esi, 0x405d20
    mov edx, 1
    mov r10d, 0x405d60
    mov r8d, 0x405d40
    mov r9d, 8
    mov eax, 441
    call syscall_expect_einval

    mov qword ptr [0x405d68], 0
    mov eax, 441
    call syscall_expect_one

    mov r10d, 0x4061fc
    mov eax, 441
    call syscall_expect_efault

    mov edi, 2
    mov esi, 0x4061fc
    mov edx, 1
    mov r10d, -1
    mov eax, 232
    call syscall_expect_efault

    mov esi, 0x405d20
    mov eax, 232
    call syscall_expect_one
    cmp dword ptr [0x405d20], 4
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x405d24], rax
    jne failure

    mov dword ptr [0x405d00], 1
    movabs rax, 0x8877665544332211
    mov qword ptr [0x405d04], rax
    mov edi, 2
    mov esi, 3
    mov edx, 1
    mov r10d, 0x405d00
    mov eax, 233
    call syscall_expect_zero

    mov esi, 0x405d20
    mov edx, 1
    xor r10d, r10d
    mov eax, 232
    call syscall_expect_zero

    mov edi, 2
    mov esi, 2
    mov edx, 1
    mov r10d, 0x406200
    mov eax, 233
    call syscall_expect_zero

    mov esi, 3
    mov r10d, 0x405d00
    mov eax, 233
    syscall
    cmp rax, -2
    jne failure

    mov edi, 2
    mov esi, 0x405d20
    mov edx, 1
    xor r10d, r10d
    mov eax, 232
    call syscall_expect_zero
    mov r10d, 1
    mov eax, 232
    call syscall_expect_enosys

    mov edi, 2
    mov eax, 3
    call syscall_expect_zero

    xor edi, edi
    xor esi, esi
    mov eax, 283
    call syscall_expect_einval
    mov edi, 1
    mov esi, 2
    mov eax, 283
    call syscall_expect_einval

    xor edi, edi
    mov eax, 291
    call syscall_expect_two
    mov edi, 1
    mov esi, 0x80800
    mov eax, 283
    call syscall_expect_three
    mov eax, 283
    syscall
    cmp rax, -24
    jne failure

    mov edi, 3
    mov esi, 1
    mov eax, 72
    call syscall_expect_one
    mov esi, 3
    mov eax, 72
    syscall
    cmp rax, 0x802
    jne failure
    mov esi, 4
    mov edx, 2
    mov eax, 72
    call syscall_expect_zero
    mov esi, 0x405e00
    mov edx, 8
    xor eax, eax
    call syscall_expect_enosys
    mov esi, 4
    mov edx, 0x802
    mov eax, 72
    call syscall_expect_zero
    mov esi, 0x405350
    mov eax, 5
    call syscall_expect_zero
    cmp dword ptr [0x405368], 0x180
    jne failure
    mov eax, 8
    syscall
    cmp rax, -29
    jne failure
    mov esi, 0x405e00
    mov edx, 8
    mov eax, 1
    call syscall_expect_einval

    mov esi, 0x405e20
    mov eax, 287
    call syscall_expect_zero
    cmp qword ptr [0x405e20], 0
    jne failure
    cmp qword ptr [0x405e38], 0
    jne failure
    mov esi, 0x405e00
    xor eax, eax
    call syscall_expect_eagain

    xor esi, esi
    mov edx, 0x406200
    xor r10d, r10d
    mov eax, 286
    call syscall_expect_efault
    mov qword ptr [0x405e00], 0
    mov qword ptr [0x405e08], 1000000000
    mov qword ptr [0x405e10], 0
    mov qword ptr [0x405e18], 1
    mov edx, 0x405e00
    mov eax, 286
    call syscall_expect_einval

    mov qword ptr [0x405e08], 0
    mov r10d, 0x405e40
    mov eax, 286
    call syscall_expect_zero
    cmp qword ptr [0x405e40], 0
    jne failure
    cmp qword ptr [0x405e58], 0
    jne failure

    mov dword ptr [0x405d00], 1
    movabs rax, 0x54494d4552464433
    mov qword ptr [0x405d04], rax
    mov edi, 2
    mov esi, 1
    mov edx, 3
    mov r10d, 0x405d00
    mov eax, 233
    call syscall_expect_zero
    mov esi, 0x405d20
    mov edx, 1
    xor r10d, r10d
    mov eax, 232
    call syscall_expect_one
    movabs rax, 0x54494d4552464433
    cmp qword ptr [0x405d24], rax
    jne failure

    mov edi, 3
    mov esi, 0x4061fc
    mov edx, 8
    xor eax, eax
    call syscall_expect_efault
    mov esi, 0x405e60
    mov edx, 8
    xor eax, eax
    call syscall_expect_eight
    cmp qword ptr [0x405e60], 1
    jne failure
    mov edi, 2
    mov esi, 0x405d20
    mov edx, 1
    xor r10d, r10d
    mov eax, 232
    call syscall_expect_zero
    mov edi, 3
    mov esi, 0x405e60
    mov edx, 8
    xor eax, eax
    call syscall_expect_eagain

    mov qword ptr [0x405e08], 1
    mov qword ptr [0x405e18], 1
    xor esi, esi
    mov edx, 0x405e00
    mov r10d, 0x405e40
    mov eax, 286
    call syscall_expect_zero
    cmp qword ptr [0x405e48], 0
    jne failure
    mov esi, 0x405e20
    mov eax, 287
    call syscall_expect_zero
    cmp qword ptr [0x405e28], 1
    jne failure
    mov esi, 0x405e60
    mov edx, 8
    xor eax, eax
    call syscall_expect_eight
    cmp qword ptr [0x405e60], 0
    je failure

    mov edi, 3
    mov qword ptr [0x405e08], 0
    mov esi, 1
    mov edx, 0x405e00
    xor r10d, r10d
    mov eax, 286
    call syscall_expect_zero
    mov esi, 0x405e60
    mov edx, 8
    xor eax, eax
    call syscall_expect_eight
    cmp qword ptr [0x405e60], 0
    je failure

    mov edi, 2
    mov esi, 2
    mov edx, 3
    xor r10d, r10d
    mov eax, 233
    call syscall_expect_zero
    mov edi, 3
    mov eax, 3
    call syscall_expect_zero
    mov edi, 2
    mov eax, 3
    call syscall_expect_zero

    mov edi, 0x406200
    xor esi, esi
    mov eax, 293
    call syscall_expect_efault
    mov edi, 0x405e80
    mov esi, 2
    mov eax, 293
    call syscall_expect_einval

    xor edi, edi
    mov eax, 291
    call syscall_expect_two
    mov edi, 0x405e80
    mov esi, 0x80800
    mov eax, 293
    call syscall_expect_zero
    cmp dword ptr [0x405e80], 3
    jne failure
    cmp dword ptr [0x405e84], 4
    jne failure
#if defined(GINUX_LINKED) && defined(GINUX_GRAPH)
    mov edi, 4
    mov esi, 0x404fff
    mov edx, 2
    mov eax, 1
    call syscall_expect_two
    mov edi, 3
    mov eax, 3
    call syscall_expect_zero
    mov edi, 4
    mov eax, 3
    call syscall_expect_zero
    mov edi, 0x405e80
    mov esi, 0x80800
    mov eax, 293
    call syscall_expect_zero
#endif
    mov eax, 293
    syscall
    cmp rax, -24
    jne failure

    mov edi, 3
    mov esi, 1
    mov eax, 72
    call syscall_expect_one
    mov esi, 3
    mov eax, 72
    syscall
    cmp rax, 0x800
    jne failure
    mov edi, 4
    mov eax, 72
    syscall
    cmp rax, 0x801
    jne failure
    mov edi, 3
    mov esi, 4
    xor edx, edx
    mov eax, 72
    call syscall_expect_zero
    mov esi, 3
    mov eax, 72
    call syscall_expect_zero
    mov edi, 4
    mov eax, 72
    syscall
    cmp rax, 0x801
    jne failure
    mov edi, 3
    mov esi, 4
    mov edx, 0x4000
    mov eax, 72
    call syscall_expect_einval
    mov esi, 0x405eb0
    mov edx, 1
    xor eax, eax
    call syscall_expect_enosys
    mov esi, 4
    mov edx, 0x800
    mov eax, 72
    call syscall_expect_zero
    mov eax, 8
    syscall
    cmp rax, -29
    jne failure

    mov dword ptr [0x405d00], 1
    movabs rax, 0x5049504552454144
    mov qword ptr [0x405d04], rax
    mov edi, 2
    mov esi, 1
    mov edx, 3
    mov r10d, 0x405d00
    mov eax, 233
    call syscall_expect_zero
    mov esi, 0x405d20
    mov edx, 1
    xor r10d, r10d
    mov eax, 232
    call syscall_expect_zero

    mov edi, 4
    mov esi, 0x406200
    mov edx, 8
    mov eax, 1
    call syscall_expect_efault

    movabs rax, 0x4847464544434241
    mov qword ptr [0x405ea0], rax
    mov edi, 4
    mov esi, 0x405ea0
    mov edx, 8
    mov eax, 1
    call syscall_expect_eight
    mov edi, 2
    mov esi, 0x405d20
    mov edx, 1
    xor r10d, r10d
    mov eax, 232
    call syscall_expect_one
    movabs rax, 0x5049504552454144
    cmp qword ptr [0x405d24], rax
    jne failure

    mov edi, 3
    mov esi, 0x405350
    mov eax, 5
    call syscall_expect_zero
    cmp dword ptr [0x405368], 0x1180
    jne failure
    mov esi, 0x541b
    mov edx, 0x405ec0
    mov eax, 16
    call syscall_expect_zero
    cmp dword ptr [0x405ec0], 8
    jne failure
    mov edx, 0x406200
    mov eax, 16
    call syscall_expect_efault

    mov edi, 3
    mov esi, 0x4061fc
    mov edx, 8
    xor eax, eax
    call syscall_expect_efault
    mov esi, 0x405eb0
    mov edx, 3
    xor eax, eax
    call syscall_expect_three
    cmp byte ptr [0x405eb0], 0x41
    jne failure
    cmp byte ptr [0x405eb2], 0x43
    jne failure
    mov esi, 0x541b
    mov edx, 0x405ec0
    mov eax, 16
    call syscall_expect_zero
    cmp dword ptr [0x405ec0], 5
    jne failure
    mov edi, 3
    mov esi, 0x405eb8
    mov edx, 5
    xor eax, eax
    call syscall_expect_five
    cmp byte ptr [0x405eb8], 0x44
    jne failure
    cmp byte ptr [0x405ebc], 0x48
    jne failure
    mov esi, 0x405eb0
    mov edx, 1
    xor eax, eax
    call syscall_expect_eagain

    mov qword ptr [0x405f00], 0x405ea0
    mov qword ptr [0x405f08], 3
    mov qword ptr [0x405f10], 0x406200
    mov qword ptr [0x405f18], 5
    mov edi, 4
    mov esi, 0x405f00
    mov edx, 2
    mov eax, 20
    call syscall_expect_efault
    mov qword ptr [0x405f10], 0x405ea3
    mov eax, 20
    call syscall_expect_eight

    mov qword ptr [0x405f20], 0x405eb0
    mov qword ptr [0x405f28], 3
    mov qword ptr [0x405f30], 0x406200
    mov qword ptr [0x405f38], 5
    mov edi, 3
    mov esi, 0x405f20
    mov eax, 19
    call syscall_expect_efault
    mov qword ptr [0x405f30], 0x405eb8
    mov eax, 19
    call syscall_expect_eight
    cmp byte ptr [0x405eb0], 0x41
    jne failure
    cmp byte ptr [0x405eb2], 0x43
    jne failure
    cmp byte ptr [0x405eb8], 0x44
    jne failure
    cmp byte ptr [0x405ebc], 0x48
    jne failure

    mov edi, 4
    mov esi, 0x405e00
    mov edx, 256
    mov eax, 1
    call syscall_expect_256
    mov edx, 1
    mov eax, 1
    call syscall_expect_eagain
    mov edi, 3
    mov esi, 0x405e00
    mov edx, 256
    xor eax, eax
    call syscall_expect_256

    mov edi, 4
    mov esi, 0x405ea0
    mov edx, 1
    mov eax, 1
    call syscall_expect_one
    mov edi, 4
    mov eax, 3
    call syscall_expect_zero
    mov dword ptr [0x405da0], 3
    mov word ptr [0x405da4], 1
    mov word ptr [0x405da6], -1
    mov edi, 0x405da0
    mov esi, 1
    xor edx, edx
    mov eax, 7
    call syscall_expect_one
    cmp word ptr [0x405da6], 0x11
    jne failure
    mov edi, 3
    mov esi, 0x405eb0
    mov edx, 1
    xor eax, eax
    call syscall_expect_one
    cmp byte ptr [0x405eb0], 0x41
    jne failure
    xor eax, eax
    call syscall_expect_zero
    mov eax, 3
    call syscall_expect_zero

    mov edi, 0x405e80
    mov eax, 22
    call syscall_expect_zero
    mov edi, 3
    mov eax, 3
    call syscall_expect_zero
    mov edi, 4
    mov esi, 0x405ea0
    mov edx, 1
    mov eax, 1
    syscall
    cmp rax, -32
    jne failure
    mov eax, 3
    call syscall_expect_zero
    mov edi, 2
    mov eax, 3
    call syscall_expect_zero

    movabs rax, 0x72657a2f7665642f
    mov qword ptr [0x405340], rax
    mov word ptr [0x405348], 0x006f
    mov edi, -100
    mov esi, 0x405340
    xor edx, edx
    mov eax, 257
    call syscall_expect_two
    mov qword ptr [0x405380], -1
    mov edi, 2
    mov esi, 0x405380
    mov edx, 8
    xor eax, eax
    call syscall_expect_eight
    cmp qword ptr [0x405380], 0
    jne failure
    mov eax, 1
    call syscall_expect_ebadf
    mov edi, -100
    mov esi, 0x405340
    mov edx, 0x405350
    xor r10d, r10d
    mov eax, 262
    call syscall_expect_zero
    mov edi, 2
    mov eax, 3
    call syscall_expect_zero

    movabs rax, 0x6e6f632f7665642f
    mov qword ptr [0x405340], rax
    mov dword ptr [0x405348], 0x656c6f73
    mov byte ptr [0x40534c], 0
    mov edi, -100
    mov esi, 0x405340
    mov edx, 1
    mov eax, 257
    call syscall_expect_two
    movabs rax, 0x2121454349564544
    mov qword ptr [0x405380], rax
    mov byte ptr [0x405388], 10
    mov edi, 2
    mov esi, 0x405380
    mov edx, 9
    mov eax, 1
    syscall
    cmp rax, 9
    jne failure
    mov esi, 0x5413
    mov edx, 0x405390
    mov eax, 16
    call syscall_expect_zero
    cmp word ptr [0x405390], 24
    jne failure
    cmp word ptr [0x405392], 80
    jne failure
    mov eax, 8
    syscall
    cmp rax, -29
    jne failure
    mov eax, 3
    call syscall_expect_zero

    movabs rax, 0x7974742f7665642f
    mov qword ptr [0x405340], rax
    mov byte ptr [0x405348], 0
    mov edi, -100
    mov esi, 0x405340
    mov edx, 2
    mov eax, 257
    call syscall_expect_two
    mov edi, 2
    mov esi, 0x406200
    mov edx, 8
    xor eax, eax
    call syscall_expect_zero
    mov eax, 3
    call syscall_expect_zero

    mov edi, -100
    mov esi, 0x405340
    xor edx, edx
    mov r10d, 0x7ff
    mov r8d, 0x406200
    mov eax, 332
    call syscall_expect_efault
    mov r8d, 0x405400
    mov eax, 332
    call syscall_expect_zero
    cmp dword ptr [0x405400], 0x7ff
    jne failure
    cmp word ptr [0x40541c], 0x2180
    jne failure
    cmp qword ptr [0x405420], 3
    jne failure
    mov r10d, 0x80000000
    mov eax, 332
    call syscall_expect_einval

    movabs rax, 0x72657a2f7665642f
    mov qword ptr [0x405340], rax
    mov word ptr [0x405348], 0x006f
    mov edi, -100
    mov esi, 0x405340
    xor edx, edx
    mov eax, 257
    call syscall_expect_two
    mov qword ptr [0x405540], -1
    mov qword ptr [0x405550], -1
    mov qword ptr [0x405500], 0x405540
    mov qword ptr [0x405508], 4
    mov qword ptr [0x405510], 0x406200
    mov qword ptr [0x405518], 4
    mov edi, 2
    mov esi, 0x405500
    mov edx, 2
    mov eax, 19
    call syscall_expect_efault
    cmp qword ptr [0x405540], -1
    jne failure
    mov qword ptr [0x405510], 0x405550
    mov eax, 19
    call syscall_expect_eight
    cmp dword ptr [0x405540], 0
    jne failure
    cmp dword ptr [0x405550], 0
    jne failure
    mov edi, 2
    mov eax, 3
    call syscall_expect_zero

    movabs rax, 0x6c756e2f7665642f
    mov qword ptr [0x405340], rax
    mov word ptr [0x405348], 0x006c
    mov edi, -100
    mov esi, 0x405340
    mov edx, 1
    mov eax, 257
    call syscall_expect_two
    mov qword ptr [0x405500], 0x406200
    mov qword ptr [0x405508], 8
    mov edi, 2
    mov esi, 0x405500
    mov edx, 1
    mov eax, 20
    call syscall_expect_eight
    mov edi, 2
    mov eax, 3
    call syscall_expect_zero

    mov dword ptr [0x405340], 0x7665642f
    mov byte ptr [0x405344], 0
    mov edi, -100
    mov esi, 0x405340
    mov edx, 0x10000
    mov eax, 257
    call syscall_expect_two
    mov edi, 2
    mov esi, 0x405700
    mov eax, 5
    call syscall_expect_zero
    cmp dword ptr [0x405718], 0x416d
    jne failure
    mov esi, 0x405800
    mov edx, 151
    mov eax, 217
    call syscall_expect_einval
    mov esi, 0x406200
    mov edx, 152
    mov eax, 217
    call syscall_expect_efault
    mov esi, 0x405800
    mov eax, 217
    call syscall_expect_152
    cmp byte ptr [0x405813], '.'
    jne failure
    cmp byte ptr [0x405843], 'n'
    jne failure
    mov eax, 217
    call syscall_expect_zero
    xor esi, esi
    xor edx, edx
    mov eax, 8
    call syscall_expect_zero
    mov esi, 0x405800
    mov edx, 152
    mov eax, 217
    call syscall_expect_152
    mov eax, 3
    call syscall_expect_zero

    movabs rax, 0x6c2f343662696c2f
    mov qword ptr [0x405340], rax
    movabs rax, 0x2e78756e69672d64
    mov qword ptr [0x405348], rax
    mov dword ptr [0x405350], 0x00006f73
    mov edi, -100
    mov esi, 0x405340
    xor edx, edx
    mov eax, 257
    call syscall_expect_two

#ifdef GINUX_LINKED
    call linked_file_mmap_test
#endif

    mov edi, 2
    mov esi, 0x405380
    mov edx, 4
    xor eax, eax
    syscall
    cmp rax, 4
    jne failure
    cmp dword ptr [0x405380], 0x464c457f
    jne failure

    mov esi, 0x405384
    mov edx, 1
    mov r10d, 0x200
    mov eax, 17
    call syscall_expect_one
    cmp byte ptr [0x405384], 0x48
    jne failure

    mov esi, 0x200
    xor edx, edx
    mov eax, 8
    syscall
    cmp rax, 0x200
    jne failure
    mov esi, 0x405384
    mov edx, 1
    xor eax, eax
    call syscall_expect_one
    cmp byte ptr [0x405384], 0x48
    jne failure

    mov esi, 0x405700
    mov eax, 5
    call syscall_expect_zero
    cmp dword ptr [0x405718], 0x816d
    jne failure
    cmp qword ptr [0x405730], 0
    je failure

    mov esi, 0x405380
    mov edx, 1
    mov eax, 1
    call syscall_expect_ebadf
    mov eax, 3
    call syscall_expect_zero

    mov edi, -100
    mov esi, 0x405340
    mov edx, 1
    mov eax, 257
    syscall
    cmp rax, -13
    jne failure
    mov edx, 0x10000
    mov eax, 257
    syscall
    cmp rax, -20
    jne failure

    mov edx, 0x405700
    xor r10d, r10d
    mov eax, 262
    call syscall_expect_zero
    cmp dword ptr [0x405718], 0x816d
    jne failure
    cmp qword ptr [0x405730], 0
    je failure

    mov edi, 0x406200
    mov eax, 80
    call syscall_expect_efault

    mov word ptr [0x4050b0], 0x2f
    mov edi, 0x4050b0
    mov eax, 80
    call syscall_expect_zero

    mov dword ptr [0x4050b0], 0x7261762f
    mov byte ptr [0x4050b4], 0
    mov edi, 0x4050b0
    mov eax, 80
    call syscall_expect_zero

    mov dword ptr [0x4050b0], 0x0062696c
    mov edi, 0x4050b0
    mov eax, 80
    call syscall_expect_zero

    mov dword ptr [0x4050b0], 0x7261762f
    mov byte ptr [0x4050b4], 0
    mov edi, 0x4050b0
    mov eax, 80
    call syscall_expect_zero

    mov dword ptr [0x4050b0], 0x0062696c
    mov edi, -100
    mov esi, 0x4050b0
    mov edx, 0x10000
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure
    mov edi, 2
    mov eax, 81
    call syscall_expect_zero
    mov eax, 3
    call syscall_expect_zero
    mov edi, 2
    mov eax, 81
    call syscall_expect_ebadf

    mov edi, 0x405020
    mov esi, 9
    mov eax, 79
    syscall
    cmp rax, 9
    jne failure
    movabs rax, 0x62696c2f7261762f
    cmp qword ptr [0x405020], rax
    jne failure
    cmp byte ptr [0x405028], 0
    jne failure

    movabs rax, 0x000062696c2f2e2e
    mov qword ptr [0x4050b0], rax
    mov edi, 0x4050b0
    mov eax, 80
    syscall
    cmp rax, -2
    jne failure

    mov word ptr [0x4050b0], 0x2f
    mov edi, 0x4050b0
    mov eax, 80
    call syscall_expect_zero

    movabs rax, 0x676e697373696d2f
    mov qword ptr [0x4050b0], rax
    mov byte ptr [0x4050b8], 0
    mov edi, 0x4050b0
    mov eax, 80
    syscall
    cmp rax, -2
    jne failure

    mov edi, 16
    mov esi, 0x4050d0
    mov eax, 157
    call syscall_expect_zero
    cmp dword ptr [0x4050d0], 0x74696e69
    jne failure
    cmp dword ptr [0x4050d4], 0
    jne failure

    mov edi, 15
    mov esi, 0x406200
    mov eax, 157
    call syscall_expect_efault

    movabs rax, 0x72702d78756e6967
    mov qword ptr [0x4050c0], rax
    mov dword ptr [0x4050c8], 0x0065626f
    mov edi, 15
    mov esi, 0x4050c0
    mov eax, 157
    call syscall_expect_zero

    mov edi, 16
    mov esi, 0x406200
    mov eax, 157
    call syscall_expect_efault

    mov edi, 16
    mov esi, 0x4050d0
    mov eax, 157
    call syscall_expect_zero
    movabs rax, 0x72702d78756e6967
    cmp qword ptr [0x4050d0], rax
    jne failure
    cmp dword ptr [0x4050d8], 0x0065626f
    jne failure
    cmp dword ptr [0x4050dc], 0
    jne failure

    mov edi, 99
    mov eax, 157
    call syscall_expect_einval

    mov dword ptr [0x4050c0], 0x74696e69
    mov byte ptr [0x4050c4], 0
    mov edi, 15
    mov esi, 0x4050c0
    mov eax, 157
    call syscall_expect_zero

    mov edi, 39
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    call syscall_expect_zero

    mov edi, 38
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    call syscall_expect_einval

    mov edi, 38
    mov esi, 1
    mov edx, 1
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    call syscall_expect_einval

    mov edi, 38
    mov esi, 1
    xor edx, edx
    mov r10d, 1
    xor r8d, r8d
    mov eax, 157
    call syscall_expect_einval

    mov edi, 39
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    call syscall_expect_zero

    mov edi, 38
    mov esi, 1
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    call syscall_expect_zero

    mov edi, 39
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    call syscall_expect_one

    mov edi, 38
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    call syscall_expect_einval

    mov edi, 39
    mov esi, 1
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    call syscall_expect_einval

    mov edi, 1
    mov esi, 0x406200
    mov rdx, -1
    xor eax, eax
    call syscall_expect_ebadf

    xor edi, edi
    mov esi, 0x406200
    mov rdx, -1
    xor eax, eax
    call syscall_expect_zero

    mov edi, 2
    mov eax, 3
    call syscall_expect_ebadf

    xor edi, edi
    mov eax, 3
    call syscall_expect_zero

    xor edi, edi
    mov eax, 3
    call syscall_expect_ebadf

    xor edi, edi
    mov esi, 0x405200
    mov eax, 5
    call syscall_expect_ebadf

    xor edi, edi
    mov esi, 0x5413
    mov edx, 0x405280
    mov eax, 16
    call syscall_expect_ebadf

    mov eax, 102
    call syscall_expect_zero
    mov eax, 104
    call syscall_expect_zero
    mov eax, 107
    call syscall_expect_zero
    mov eax, 108
    call syscall_expect_zero

    mov rdi, -1
    xor esi, esi
    mov eax, 115
    call syscall_expect_einval

    xor edi, edi
    mov esi, 0x406200
    mov eax, 115
    call syscall_expect_one

    mov dword ptr [0x405060], 0x12345678
    mov edi, 1
    mov esi, 0x406200
    mov eax, 115
    call syscall_expect_efault
    cmp dword ptr [0x405060], 0x12345678
    jne failure

    mov edi, 1
    mov esi, 0x405060
    mov eax, 115
    call syscall_expect_one
    cmp dword ptr [0x405060], 0
    jne failure

    mov edi, -1
    mov eax, 135
    call syscall_expect_zero

    xor edi, edi
    mov eax, 135
    call syscall_expect_zero

    mov edi, 0x40000
    mov eax, 135
    call syscall_expect_einval

    mov dword ptr [0x405060], 0x20080522
    mov dword ptr [0x405064], 0
    mov dword ptr [0x405068], 0x12345678
    mov edi, 0x405060
    mov esi, 0x406200
    mov eax, 125
    call syscall_expect_efault
    cmp dword ptr [0x405068], 0x12345678
    jne failure

    mov edi, 0x405060
    mov esi, 0x405068
    mov eax, 125
    call syscall_expect_zero
    mov eax, dword ptr [0x405068]
    or eax, dword ptr [0x40506c]
    or eax, dword ptr [0x405070]
    or eax, dword ptr [0x405074]
    or eax, dword ptr [0x405078]
    or eax, dword ptr [0x40507c]
    jne failure

    mov dword ptr [0x405060], 0
    mov edi, 0x405060
    xor esi, esi
    mov eax, 125
    call syscall_expect_einval
    cmp dword ptr [0x405060], 0x20080522
    jne failure

    mov dword ptr [0x405064], 2
    mov eax, 125
    syscall
    cmp rax, -3
    jne failure

    mov dword ptr [0x405064], 0
    mov edi, 0x405060
    mov esi, 0x405068
    mov eax, 126
    call syscall_expect_zero
    mov dword ptr [0x405068], 1
    mov eax, 126
    syscall
    cmp rax, -1
    jne failure
    mov dword ptr [0x405068], 0

    xor edi, edi
    xor esi, esi
    mov eax, 116
    syscall
    cmp rax, -1
    jne failure

    mov dword ptr [0x405060], 0x12345678
    mov dword ptr [0x405064], 0x12345678
    mov edi, 0x405060
    mov esi, 0x405064
    mov edx, 0x406200
    mov eax, 118
    call syscall_expect_efault
    cmp dword ptr [0x405060], 0x12345678
    jne failure
    cmp dword ptr [0x405064], 0x12345678
    jne failure

    mov edi, 0x405060
    mov esi, 0x405064
    mov edx, 0x405068
    mov eax, 118
    call syscall_expect_zero
    cmp dword ptr [0x405060], 0
    jne failure
    cmp dword ptr [0x405064], 0
    jne failure
    cmp dword ptr [0x405068], 0
    jne failure

    mov dword ptr [0x405060], 0x12345678
    mov dword ptr [0x405064], 0x12345678
    mov dword ptr [0x405068], 0x12345678
    mov edi, 0x405060
    mov esi, 0x405064
    mov edx, 0x405068
    mov eax, 120
    call syscall_expect_zero
    cmp dword ptr [0x405060], 0
    jne failure
    cmp dword ptr [0x405064], 0
    jne failure
    cmp dword ptr [0x405068], 0
    jne failure

    mov eax, 186
    call syscall_expect_one

    mov eax, 110
    call syscall_expect_zero

    mov eax, 111
    call syscall_expect_one

    xor edi, edi
    mov eax, 121
    call syscall_expect_one
    mov edi, 2
    mov eax, 121
    syscall
    cmp rax, -3
    jne failure

    xor edi, edi
    mov eax, 124
    call syscall_expect_one
    mov edi, 2
    mov eax, 124
    syscall
    cmp rax, -3
    jne failure

    mov edi, 0x4051b0
    mov esi, 23
    mov eax, 273
    call syscall_expect_einval

    mov edi, 0x4061f0
    mov esi, 24
    mov eax, 273
    call syscall_expect_efault

    mov edi, 0x4051b0
    mov esi, 24
    mov eax, 273
    call syscall_expect_zero

    mov edi, 2
    mov esi, 0x4051c8
    mov edx, 0x4051d0
    mov eax, 274
    syscall
    cmp rax, -3
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4051c8], rax
    xor edi, edi
    mov esi, 0x4051c8
    mov edx, 0x4061fc
    mov eax, 274
    call syscall_expect_efault
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4051c8], rax
    jne failure

    xor edi, edi
    mov esi, 0x4051c8
    mov edx, 0x4051d0
    mov eax, 274
    call syscall_expect_zero
    cmp qword ptr [0x4051c8], 0x4051b0
    jne failure
    cmp qword ptr [0x4051d0], 24
    jne failure

    mov edi, 0x406001
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    call syscall_expect_einval

    mov edi, 0x406000
    mov esi, 4096
    mov edx, 4
    mov eax, 10
    call syscall_expect_einval

    movabs rax, 0x0a212145424f5250
    mov qword ptr [0x406000], rax
    mov edi, 0x406000
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    call syscall_expect_zero

    mov edi, 0x406000
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    call syscall_expect_einval

    mov edi, 0x406000
    mov eax, 63
    call syscall_expect_efault

    mov edi, 1
    mov esi, 0x406000
    mov edx, 8
    mov eax, 1
    call syscall_expect_eight

    movabs rax, 0x1122334455667788
    mov qword ptr [0x405280], rax
    mov qword ptr [0x405298], rax
    mov edi, 1
    mov esi, 0x5401
    mov edx, 0x405280
    mov eax, 16
    call syscall_expect_zero
    cmp qword ptr [0x405280], 0
    jne failure
    cmp qword ptr [0x405298], 0
    jne failure

    mov edi, 2
    mov eax, 32
    call syscall_expect_ebadf

    mov edi, 2
    xor esi, esi
    xor edx, edx
    mov eax, 72
    call syscall_expect_ebadf

    mov edi, 1
    xor esi, esi
    mov edx, 2
    mov eax, 72
    call syscall_expect_einval

    mov edi, 1
    xor esi, esi
    xor edx, edx
    mov eax, 72
    call syscall_expect_zero

    xor edi, edi
    mov esi, 2
    mov edx, 1
    mov eax, 72
    call syscall_expect_zero

    xor edi, edi
    mov esi, 1
    mov eax, 72
    call syscall_expect_one

    xor edi, edi
    mov esi, 2
    mov edx, 2
    mov eax, 72
    call syscall_expect_zero

    xor edi, edi
    mov esi, 1
    mov eax, 72
    call syscall_expect_zero

    mov edi, 1
    xor esi, esi
    xor edx, edx
    mov eax, 72
    syscall
    cmp rax, -24
    jne failure

    xor edi, edi
    mov eax, 3
    call syscall_expect_zero

    mov edi, 1
    mov esi, 1030
    xor edx, edx
    mov eax, 72
    call syscall_expect_zero

    xor edi, edi
    mov esi, 1
    mov eax, 72
    call syscall_expect_one

    xor edi, edi
    mov eax, 3
    call syscall_expect_zero

    mov edi, 1
    mov eax, 32
    call syscall_expect_zero

    xor edi, edi
    mov esi, 1
    mov eax, 72
    call syscall_expect_zero

    mov edi, 1
    mov eax, 32
    syscall
    cmp rax, -24
    jne failure

    xor edi, edi
    lea rsi, [rip + readable_payload]
    mov edx, 8
    mov eax, 1
    call syscall_expect_eight

    xor edi, edi
    xor esi, esi
    mov eax, 33
    call syscall_expect_zero

    xor edi, edi
    mov esi, 1
    mov eax, 33
    call syscall_expect_one

    xor edi, edi
    mov esi, 2
    mov eax, 33
    call syscall_expect_ebadf

    mov edi, 2
    mov esi, 1
    mov eax, 33
    call syscall_expect_ebadf

    xor edi, edi
    mov esi, 3
    mov eax, 72
    call syscall_expect_one

    mov edi, 1
    mov esi, 1
    mov eax, 72
    call syscall_expect_zero

    mov edi, 1
    mov eax, 3
    call syscall_expect_zero

    xor edi, edi
    mov eax, 3
    call syscall_expect_zero

    movabs rax, 0x1122334455667788
    mov qword ptr [0x405ff8], rax
    mov edi, 1
    mov esi, 0x405ff8
    mov eax, 228
    call syscall_expect_efault
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x405ff8], rax
    jne failure

    xor edi, edi
    mov esi, 0x405900
    mov eax, 228
    call syscall_expect_einval

    mov edi, 1
    mov esi, 0x405900
    mov eax, 228
    call syscall_expect_zero
    cmp qword ptr [0x405900], 0
    jl failure
    cmp qword ptr [0x405908], 1000000000
    jae failure

    mov edi, 1
    mov esi, 0x405910
    mov eax, 228
    call syscall_expect_zero
    mov rax, qword ptr [0x405910]
    cmp rax, qword ptr [0x405900]
    jb failure
    ja clock_advanced
    mov rax, qword ptr [0x405918]
    cmp rax, qword ptr [0x405908]
    jb failure
clock_advanced:
    cmp qword ptr [0x405918], 1000000000
    jae failure

    mov edi, 4
    mov esi, 0x405920
    mov eax, 228
    call syscall_expect_zero
    cmp qword ptr [0x405928], 1000000000
    jae failure

    mov edi, 2
    mov esi, 0x405950
    mov eax, 228
    call syscall_expect_zero
    cmp qword ptr [0x405958], 1000000000
    jae failure
    mov edi, 3
    mov esi, 0x405960
    mov eax, 228
    call syscall_expect_zero
    cmp qword ptr [0x405968], 1000000000
    jae failure

    xor edi, edi
    xor esi, esi
    mov eax, 229
    call syscall_expect_einval

    mov edi, 1
    xor esi, esi
    mov eax, 229
    call syscall_expect_zero

    movabs rax, 0x1122334455667788
    mov qword ptr [0x405ff8], rax
    mov edi, 4
    mov esi, 0x405ff8
    mov eax, 229
    call syscall_expect_efault
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x405ff8], rax
    jne failure

    mov edi, 1
    mov esi, 0x405930
    mov eax, 229
    call syscall_expect_zero
    cmp qword ptr [0x405930], 0
    je resolution_subsecond
    cmp qword ptr [0x405930], 1
    jne failure
    cmp qword ptr [0x405938], 0
    jne failure
    jmp resolution_valid
resolution_subsecond:
    cmp qword ptr [0x405938], 0
    jbe failure
    cmp qword ptr [0x405938], 1000000000
    jae failure
resolution_valid:

    movabs rax, 0x1122334455667788
    mov qword ptr [0x405ff0], rax
    mov edi, 0x405ff0
    mov eax, 100
    call syscall_expect_efault
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x405ff0], rax
    jne failure
    xor edi, edi
    mov eax, 100
    syscall
    test rax, rax
    js failure
    mov edi, 0x405970
    mov eax, 100
    syscall
    test rax, rax
    js failure
    cmp qword ptr [0x405970], rax
    jne failure
    cmp qword ptr [0x405978], 0
    jne failure
    cmp qword ptr [0x405980], 0
    jne failure
    cmp qword ptr [0x405988], 0
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x405ff0], rax
    xor edi, edi
    mov esi, 0x405ff0
    mov eax, 36
    call syscall_expect_efault
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x405ff0], rax
    jne failure

    mov edi, 3
    mov esi, 0x405bc0
    mov eax, 36
    call syscall_expect_einval

    xor edi, edi
    mov esi, 0x405bc0
    mov eax, 36
    call syscall_expect_zero
    cmp qword ptr [0x405bc0], 0
    jne failure
    cmp qword ptr [0x405bd8], 0
    jne failure

    mov qword ptr [0x405be0], 1
    mov qword ptr [0x405be8], 0
    mov qword ptr [0x405bf0], 0
    mov qword ptr [0x405bf8], 0
    movabs rax, 0x1122334455667788
    mov qword ptr [0x405c00], rax
    xor edi, edi
    mov esi, 0x405be0
    mov edx, 0x405c00
    mov eax, 38
    call syscall_expect_enosys
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x405c00], rax
    jne failure

    mov qword ptr [0x405be0], 0
    xor edi, edi
    mov esi, 0x405be0
    mov edx, 0x405c00
    mov eax, 38
    call syscall_expect_zero
    cmp qword ptr [0x405c00], 0
    jne failure
    cmp qword ptr [0x405c18], 0
    jne failure

    xor edi, edi
    mov esi, 0x406200
    xor edx, edx
    mov eax, 38
    call syscall_expect_efault

    mov dword ptr [0x405c40], 42
    mov edi, 0x405c40
    mov esi, 2
    mov edx, 42
    mov eax, 202
    call syscall_expect_einval

    mov edi, 0x405c41
    xor esi, esi
    mov eax, 202
    call syscall_expect_einval
    mov edi, 0x406200
    mov eax, 202
    call syscall_expect_efault

    mov edi, 0x405c40
    mov edx, 41
    mov eax, 202
    call syscall_expect_eagain
    mov esi, 128
    mov eax, 202
    call syscall_expect_eagain

    xor esi, esi
    mov edx, 42
    mov eax, 202
    call syscall_expect_enosys

    movabs rax, 0x1122334455667788
    mov qword ptr [0x405ff0], rax
    mov qword ptr [0x405ff8], rax
    xor edi, edi
    mov esi, 0x405ff0
    mov eax, 98
    call syscall_expect_efault
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x405ff0], rax
    jne failure
    cmp qword ptr [0x405ff8], rax
    jne failure

    mov edi, 2
    mov esi, 0x405a00
    mov eax, 98
    call syscall_expect_einval
    xor edi, edi
    mov eax, 98
    call syscall_expect_zero
    cmp qword ptr [0x405a08], 1000000
    jae failure
    cmp qword ptr [0x405a10], 0
    jne failure
    cmp qword ptr [0x405a18], 0
    jne failure

    mov edi, 1
    mov esi, 0x405a90
    mov eax, 98
    call syscall_expect_zero
    cmp qword ptr [0x405a98], 1000000
    jae failure

    mov edi, -1
    mov esi, 0x405b20
    mov eax, 98
    call syscall_expect_zero
    cmp qword ptr [0x405b20], 0
    jne failure
    cmp qword ptr [0x405ba8], 0
    jne failure

    mov edi, 0x406200
    mov eax, 218
    call syscall_expect_efault

    mov dword ptr [0x405190], 0x12345678
    mov edi, 0x405190
    mov eax, 218
    call syscall_expect_one

    mov r10d, 0x12345678
    mov edi, 0x11223344
    mov eax, 39
    call syscall_expect_one
    cmp r10d, 0x12345678
    jne failure
    cmp edi, 0x11223344
    jne failure

    call nested_getpid
    cmp rax, 1
    jne failure

#if defined(GINUX_LINKED) && defined(GINUX_GRAPH)
    mov edi, 0x13d0f00
    mov esi, 0x411000
    mov edx, 0x4061fc
    mov r8d, 0x405198
    mov r9d, 0x40efc0
    mov eax, 56
    call syscall_expect_efault
#endif

#ifndef GINUX_LINKED
    sub rsp, 192
    lea rax, [rip + exec_path]
    mov qword ptr [rsp], rax
    lea rax, [rip + exec_mode]
    mov qword ptr [rsp + 8], rax
    mov qword ptr [rsp + 16], 0
    lea rax, [rip + exec_environment]
    mov qword ptr [rsp + 24], rax
    mov qword ptr [rsp + 32], 0

    mov edi, -100
    lea rsi, [rip + exec_zero_path]
    mov edx, 0x80000
    mov eax, 257
    call syscall_expect_two

    mov edi, -100
    lea rsi, [rip + exec_null_path]
    mov edx, 1
    mov eax, 257
    call syscall_expect_three

    mov qword ptr [rsp + 80], 0x400100
    mov qword ptr [rsp + 88], 0
    mov qword ptr [rsp + 96], 0
    mov qword ptr [rsp + 104], 0
    mov edi, 10
    lea rsi, [rsp + 80]
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    call syscall_expect_zero

    mov qword ptr [rsp + 80], 1
    mov edi, 12
    lea rsi, [rsp + 80]
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    call syscall_expect_zero

    mov qword ptr [rsp + 112], 0x405800
    mov qword ptr [rsp + 120], 0
    mov qword ptr [rsp + 128], 2048
    lea rdi, [rsp + 112]
    xor esi, esi
    mov eax, 131
    call syscall_expect_zero

    mov edi, 15
    lea rsi, [rip + pre_exec_name]
    mov eax, 157
    call syscall_expect_zero

    lea rdi, [rip + missing_exec_path]
    mov rsi, rsp
    lea rdx, [rsp + 24]
    mov eax, 59
    call syscall_expect_enoent

    lea rdi, [rip + exec_directory_path]
    mov rsi, rsp
    lea rdx, [rsp + 24]
    mov eax, 59
    syscall
    cmp rax, -13
    jne failure

    lea rdi, [rip + invalid_exec_path]
    mov rsi, rsp
    lea rdx, [rsp + 24]
    mov eax, 59
    syscall
    cmp rax, -8
    jne failure

    mov edi, 0x406200
    mov rsi, rsp
    lea rdx, [rsp + 24]
    mov eax, 59
    call syscall_expect_efault

    mov qword ptr [rsp + 64], 0
    mov qword ptr [rsp + 72], 0
    lea rdi, [rip + exec_path]
    lea rsi, [rsp + 64]
    lea rdx, [rsp + 24]
    mov eax, 59
    call syscall_expect_e2big

    lea rax, [rip + exec_mode]
    mov qword ptr [rsp + 64], rax
    mov qword ptr [rsp + 72], rax
    mov qword ptr [rsp + 80], rax
    mov qword ptr [rsp + 88], rax
    mov qword ptr [rsp + 96], rax
    mov qword ptr [rsp + 104], rax
    mov qword ptr [rsp + 112], rax
    mov qword ptr [rsp + 120], rax
    mov qword ptr [rsp + 128], rax
    lea rdi, [rip + exec_path]
    lea rsi, [rsp + 64]
    lea rdx, [rsp + 24]
    mov eax, 59
    call syscall_expect_e2big

    lea rax, [rip + long_exec_value]
    mov qword ptr [0x405c80], rax
    mov qword ptr [0x405c88], rax
    mov qword ptr [0x405c90], rax
    mov qword ptr [0x405c98], rax
    mov qword ptr [0x405ca0], rax
    mov qword ptr [0x405ca8], rax
    mov qword ptr [0x405cb0], rax
    mov qword ptr [0x405cb8], rax
    mov qword ptr [0x405cc0], 0
    mov qword ptr [0x405cd0], rax
    mov qword ptr [0x405cd8], rax
    mov qword ptr [0x405ce0], rax
    mov qword ptr [0x405ce8], rax
    mov qword ptr [0x405cf0], rax
    mov qword ptr [0x405cf8], rax
    mov qword ptr [0x405d00], rax
    mov qword ptr [0x405d08], rax
    mov qword ptr [0x405d10], 0
    lea rdi, [rip + exec_path]
    mov esi, 0x405c80
    mov edx, 0x405cd0
    mov eax, 59
    call syscall_expect_e2big

    mov edi, 2
    mov esi, 1
    xor edx, edx
    mov eax, 72
    call syscall_expect_one

    lea rdi, [rip + proc_self_exe]
    lea rsi, [rsp + 144]
    mov edx, 32
    mov eax, 89
    call syscall_expect_five
    cmp dword ptr [rsp + 144], 0x696e692f
    jne failure
    cmp byte ptr [rsp + 148], 't'
    jne failure

    mov edi, 99
    lea rsi, [rip + exec_mode]
    mov rdx, rsp
    lea r10, [rsp + 24]
    xor r8d, r8d
    mov eax, 322
    call syscall_expect_ebadf

    mov edi, 2
    mov eax, 322
    syscall
    cmp rax, -20
    jne failure

    mov edi, -100
    mov r8d, 1
    mov eax, 322
    call syscall_expect_einval

    mov edi, 3
    mov eax, 3
    call syscall_expect_zero

    mov edi, -100
    lea rsi, [rip + exec_directory_path]
    mov edx, 0x10000
    mov eax, 257
    call syscall_expect_three

    lea rdi, [rip + exec_root_path]
    mov eax, 80
    call syscall_expect_zero

    mov edi, 3
    lea rsi, [rip + missing_relative_exec_path]
    mov rdx, rsp
    lea r10, [rsp + 24]
    xor r8d, r8d
    mov eax, 322
    call syscall_expect_enoent

    mov edi, 3
    lea rsi, [rip + empty_exec_path]
    mov rdx, rsp
    lea r10, [rsp + 24]
    mov r8d, 0x1000
    mov eax, 322
    syscall
    cmp rax, -13
    jne failure

    mov edi, 3
    mov eax, 3
    call syscall_expect_zero

    mov edi, -100
    lea rsi, [rip + exec_path]
    xor edx, edx
    mov eax, 257
    call syscall_expect_three

    mov edi, 3
    lea rsi, [rip + empty_exec_path]
    mov rdx, rsp
    lea r10, [rsp + 24]
    mov r8d, 0x1000
    mov eax, 322
    syscall
    jmp failure
#elif defined(GINUX_GRAPH)
    sub rsp, 64
    lea rax, [rip + .Ldynamic_exec_path]
    mov qword ptr [rsp], rax
    lea rax, [rip + .Ldynamic_exec_argument]
    mov qword ptr [rsp + 8], rax
    mov qword ptr [rsp + 16], 0
    lea rax, [rip + .Ldynamic_exec_environment]
    mov qword ptr [rsp + 24], rax
    lea rax, [rip + .Ldynamic_exec_environment_second]
    mov qword ptr [rsp + 32], rax
    mov qword ptr [rsp + 40], 0
    lea rdi, [rip + .Ldynamic_exec_path]
    mov rsi, rsp
    lea rdx, [rsp + 24]
    mov eax, 59
    syscall
    jmp failure
#else
    mov edi, 42
    mov eax, 231
    syscall
#endif

#if defined(GINUX_LINKED) && defined(GINUX_GRAPH)
.Ldynamic_exec_complete:
    sub rsp, 16
    lea rdi, [rip + proc_self_exe]
    mov rsi, rsp
    mov edx, 16
    mov eax, 89
    syscall
    cmp rax, 5
    jne failure
    cmp dword ptr [rsp], 0x696e692f
    jne failure
    cmp byte ptr [rsp + 4], 0x74
    jne failure
    add rsp, 16
    mov edi, 0x406000
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    call syscall_expect_zero
    mov edi, 42
    mov eax, 231
    syscall

child_thread_probe:
    mov eax, 186
    syscall
    cmp eax, dword ptr [r8]
    jne failure
    mov rax, qword ptr fs:[0]
    cmp rax, r9
    jne failure
    mov eax, 24
    call syscall_expect_zero
    mov rdi, rdx
    xor esi, esi
    mov edx, dword ptr [rdi]
    mov eax, 202
    call syscall_expect_zero
    mov edi, 42
    mov eax, 60
    syscall
#endif
failure:
    ud2

syscall_expect_zero:
    syscall
    test rax, rax
    jne failure
    ret

syscall_expect_efault:
    syscall
    cmp rax, -14
    jne failure
    ret

syscall_expect_einval:
    syscall
    cmp rax, -22
    jne failure
    ret

syscall_expect_ebadf:
    syscall
    cmp rax, -9
    jne failure
    ret

syscall_expect_enosys:
    syscall
    cmp rax, -38
    jne failure
    ret

syscall_expect_eagain:
    syscall
    cmp rax, -11
    jne failure
    ret

#ifndef GINUX_LINKED
syscall_expect_enoent:
    syscall
    cmp rax, -2
    jne failure
    ret

syscall_expect_e2big:
    syscall
    cmp rax, -7
    jne failure
    ret
#endif

syscall_expect_one:
    syscall
    cmp rax, 1
    jne failure
    ret

syscall_expect_two:
    syscall
    cmp rax, 2
    jne failure
    ret

syscall_expect_three:
    syscall
    cmp rax, 3
    jne failure
    ret

syscall_expect_five:
    syscall
    cmp rax, 5
    jne failure
    ret

syscall_expect_eight:
    syscall
    cmp rax, 8
    jne failure
    ret

syscall_expect_152:
    syscall
    cmp rax, 152
    jne failure
    ret

syscall_expect_256:
    syscall
    cmp rax, 256
    jne failure
    ret

nested_getpid:
    mov eax, 39
    syscall
    ret

#ifndef GINUX_LINKED
exec_path:
#ifdef GINUX_EXEC_SECOND
    .asciz "/bin/exec-second"
#else
    .asciz "/bin/exec-target"
#endif
exec_mode:
#ifdef GINUX_EXEC_SECOND
    .asciz "exec-second"
#else
    .asciz "exec-target"
#endif
exec_environment:
    .asciz "GINUX_EXEC=1"
missing_exec_path:
    .asciz "/bin/missing"
missing_relative_exec_path:
    .asciz "missing"
exec_directory_path:
    .asciz "/bin"
exec_root_path:
    .asciz "/"
empty_exec_path:
    .byte 0
invalid_exec_path:
    .asciz "/etc/hostname"
exec_zero_path:
    .asciz "/dev/zero"
exec_null_path:
    .asciz "/dev/null"
pre_exec_name:
    .asciz "pre-exec"
long_exec_value:
    .ascii "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    .byte 0
#endif

#ifdef GINUX_LINKED
.section .text.file_mmap,"ax",@progbits
linked_file_mmap_test:
    xor edi, edi
    mov esi, 8192
    mov edx, 1
    mov r10d, 2
    mov r8d, 2
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40b000
    jne failure
    cmp dword ptr [0x40b000], 0x464c457f
    jne failure
    cmp byte ptr [0x40b200], 0x48
    jne failure
    cmp byte ptr [0x40cfff], 0
    jne failure

    mov edi, 2
    mov esi, 0x40b000
    mov edx, 1
    xor r10d, r10d
    mov eax, 17
    call syscall_expect_efault

    mov edi, 0x40b000
    mov esi, 8192
    mov eax, 11
    call syscall_expect_zero

#ifndef GINUX_GRAPH
    xor edi, edi
    mov esi, 4096
    mov edx, 1
    mov r10d, 2
    mov r8d, 2
    mov r9d, 1
    mov eax, 9
    call syscall_expect_einval
#endif
    ret
.section .text
#endif

#ifdef GINUX_GRAPH
.section .text.file_mmap,"ax",@progbits
.global ginux_linux_init
.hidden ginux_linux_init
.type ginux_linux_init,@function
ginux_linux_init:
    jmp .Linitialize_main_tls

.type ginux_linux_initialize,@function
ginux_linux_initialize:
    cmp qword ptr [rip + ginux_main_init_state], 2
    jne .Lmain_initialize_done
    call .Lvalidate_main_startup
    test eax, eax
    jz .Lmain_initialize_done
    mov rax, qword ptr [rip + ginux_dependency_symbol@GOTPCREL]
    test rax, rax
    jz .Lmain_initialize_done
    call ginux_dependency_symbol@PLT
    movabs rcx, 0x59434e45444e4947
    cmp rax, rcx
    jne .Lmain_initialize_done
    mov rax, qword ptr [rip + ginux_auxiliary_symbol@GOTPCREL]
    test rax, rax
    jz .Lmain_initialize_done
    call ginux_auxiliary_symbol@PLT
    movabs rcx, 0x5952414958554147
    cmp rax, rcx
    jne .Lmain_initialize_done
    movabs rax, 0x4e49414d54494e49
    mov qword ptr [rip + ginux_main_init_state], rax
.Lmain_initialize_done:
    ret
.section .text

.Linitialize_main_tls:
    mov rax, qword ptr fs:[0]
    test rax, rax
    jz .Lmain_init_done
    cmp qword ptr [rax], rax
    jne .Lmain_init_done
    movabs rcx, 0x0a214b4f5f534c54
    cmp qword ptr fs:[ginux_main_tls_template@TPOFF], rcx
    jne .Lmain_init_done
    cmp qword ptr fs:[ginux_main_tls_zero@TPOFF], 0
    jne .Lmain_init_done
    inc qword ptr [rip + ginux_main_init_state]
.Lmain_init_done:
    ret

.Lvalidate_main_startup:
    xor eax, eax
    cmp edi, 1
    je .Lvalidate_main_startup_single_vector
    cmp edi, 2
    jne .Lvalidate_main_startup_done
    cmp qword ptr [rsi + 16], 0
    jne .Lvalidate_main_startup_done
    cmp qword ptr [rdx + 16], 0
    jne .Lvalidate_main_startup_done
    mov r8, qword ptr [rsi + 8]
    movabs r9, 0x2d63696d616e7964
    cmp qword ptr [r8], r9
    jne .Lvalidate_main_startup_done
    cmp dword ptr [r8 + 8], 0x63657865
    jne .Lvalidate_main_startup_done
    cmp byte ptr [r8 + 12], 0
    jne .Lvalidate_main_startup_done
    mov r8, qword ptr [rdx]
    movabs r9, 0x00313d58554e4947
    cmp qword ptr [r8], r9
    jne .Lvalidate_main_startup_done
    mov r8, qword ptr [rdx + 8]
    movabs r9, 0x4f4d5f58554e4947
    cmp qword ptr [r8], r9
    jne .Lvalidate_main_startup_done
    movabs r9, 0x00636578653d4544
    cmp qword ptr [r8 + 8], r9
    jne .Lvalidate_main_startup_done
    jmp .Lvalidate_main_startup_common
.Lvalidate_main_startup_single_vector:
    cmp qword ptr [rsi + 8], 0
    jne .Lvalidate_main_startup_done
    cmp qword ptr [rdx + 8], 0
    jne .Lvalidate_main_startup_done
.Lvalidate_main_startup_common:
    mov r8, qword ptr [rsi]
    cmp dword ptr [r8], 0x696e692f
    jne .Lvalidate_main_startup_done
    cmp word ptr [r8 + 4], 0x74
    jne .Lvalidate_main_startup_done
    mov r8, qword ptr [rdx]
    movabs r9, 0x00313d58554e4947
    cmp qword ptr [r8], r9
    jne .Lvalidate_main_startup_done
    mov eax, 1
.Lvalidate_main_startup_done:
    ret
#endif

.balign 8
readable_payload:
    .ascii "PROBE!!\n"
proc_self_exe:
    .asciz "/proc/self/exe"
#ifdef GINUX_LINKED
auxiliary_dependency_path:
    .asciz "/lib64/libginux-aux.so"
#endif
#if defined(GINUX_LINKED) && defined(GINUX_GRAPH)
.Ldynamic_exec_path:
    .asciz "/init"
.Ldynamic_exec_argument:
    .asciz "dynamic-exec"
.Ldynamic_exec_environment:
    .asciz "GINUX=1"
.Ldynamic_exec_environment_second:
    .asciz "GINUX_MODE=exec"
#endif

.section .data
.balign 8
.global ginux_linux_probe_data
ginux_linux_probe_data:
    .ascii "PROBE!!\n"
#ifdef GINUX_GRAPH
.global ginux_main_init_state
ginux_main_init_state:
    .quad 0
#endif

#if defined(GINUX_LINKED) && defined(GINUX_GRAPH)
.section .tdata,"awT",@progbits
.balign 8
.global ginux_main_tls_template
.hidden ginux_main_tls_template
ginux_main_tls_template:
    .ascii "TLS_OK!\n"

.section .tbss,"awT",@nobits
.balign 8
.global ginux_main_tls_zero
.hidden ginux_main_tls_zero
ginux_main_tls_zero:
    .space 8
#endif

.section .bss
.balign 8
#ifdef GINUX_GRAPH
    .space 4064
#else
    .space 4088
#endif

#if defined(GINUX_LINKED) && defined(GINUX_GRAPH)
.section .data.rel.ro.head,"aw",@progbits
.balign 8
    .space 256

.section .preinit_array,"aw",@preinit_array
.balign 8
    .quad ginux_linux_init

.section .init_array,"aw",@init_array
.balign 8
    .quad ginux_linux_initialize

.section .data.rel.ro.tail,"aw",@progbits
.balign 8
    .space 240
#else
.section .data.rel.ro,"aw",@progbits
.balign 8
#ifdef GINUX_LINKED
    .space 256
    .quad ginux_linux_probe
    .space 248
#else
    .space 256
.global ginux_main_relative_slot
ginux_main_relative_slot:
    .quad 0
.global ginux_main_glob_dat_slot
ginux_main_glob_dat_slot:
    .quad 0
.global ginux_main_jump_slot
ginux_main_jump_slot:
    .quad 0
    .space 232
#endif
#endif

.intel_syntax noprefix
.section .text
.global ginux_linux_probe
ginux_linux_probe:
    cmp qword ptr [rsp], 1
    jne failure
    mov rax, qword ptr [rsp + 8]
    cmp byte ptr [rax], '/'
    jne failure
    mov rax, qword ptr [rsp + 24]
    cmp byte ptr [rax], 'G'
    jne failure
    cmp qword ptr [rsp + 112], 0
    jne failure
    cmp qword ptr [rsp + 128], 0
    jne failure
    cmp qword ptr [rsp + 208], 0
    jne failure
    mov rax, qword ptr [rsp + 224]
    cmp byte ptr [rax], '/'
    jne failure
    cmp qword ptr [rsp + 240], 0x400100
    jne failure

    mov edi, 1
    lea rsi, [rip + readable_payload]
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure

    mov edi, 0x400000
    mov eax, 63
    syscall
    cmp rax, -14
    jne failure

    movabs rax, 0x0807060504030201
    mov qword ptr [rsp - 16], rax
    mov edi, 1
    lea rsi, [rsp - 16]
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure
    mov edi, 1
    mov esi, 0x405ff9
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, -14
    jne failure

    xor edi, edi
    mov eax, 12
    syscall
    cmp rax, 0x403000
    jne failure
    mov edi, 0x403080
    mov eax, 12
    syscall
    cmp rax, 0x403080
    jne failure
    movabs rax, 0x0807060504030201
    mov qword ptr [0x403000], rax
    mov edi, 1
    mov esi, 0x403000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure
    mov edi, 1
    mov esi, 0x403079
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x4845415042524b21
    mov qword ptr [0x403000], rax
    mov edi, 0x404001
    mov eax, 12
    syscall
    cmp rax, 0x403080
    jne failure
    mov edi, 0x403040
    mov eax, 12
    syscall
    cmp rax, 0x403040
    jne failure

    mov eax, 999
    syscall
    cmp rax, -38
    jne failure

    mov edi, 2
    mov esi, 0x401000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, -9
    jne failure

    mov edi, 1
    mov esi, 0x401000
    mov edx, 7
    mov eax, 1
    syscall
    cmp rax, -22
    jne failure

    mov edi, 1
    mov esi, 0x401000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure

    mov edi, 0x1002
    mov esi, 0x401000
    mov eax, 158
    syscall
    test rax, rax
    jne failure
    mov rbx, qword ptr fs:[0]
    movabs rax, 0x0807060504030201
    cmp rbx, rax
    jne failure

    mov edi, 0x1003
    mov esi, 0x402200
    mov eax, 158
    syscall
    cmp rax, -14
    jne failure

    xor edi, edi
    xor esi, esi
    mov eax, 158
    syscall
    cmp rax, -22
    jne failure

    mov edi, 0x1003
    mov esi, 0x401008
    mov eax, 158
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x401008], 0x401000
    jne failure

    mov edi, 0x401020
    mov esi, 1
    mov eax, 79
    syscall
    cmp rax, -34
    jne failure
    mov edi, 0x402200
    mov esi, 2
    mov eax, 79
    syscall
    cmp rax, -14
    jne failure
    mov edi, 0x401020
    mov esi, 2
    mov eax, 79
    syscall
    cmp rax, 2
    jne failure
    cmp word ptr [0x401020], 0x002f
    jne failure

    lea rdi, [rip + proc_self_exe]
    mov esi, 0x401040
    xor edx, edx
    mov eax, 89
    syscall
    cmp rax, -22
    jne failure
    mov edi, 0x406000
    mov esi, 0x401040
    mov edx, 5
    mov eax, 89
    syscall
    cmp rax, -14
    jne failure
    lea rdi, [rip + proc_self_exe]
    mov esi, 0x402200
    mov edx, 5
    mov eax, 89
    syscall
    cmp rax, -14
    jne failure
    lea rdi, [rip + proc_self_exe]
    mov esi, 0x401040
    mov edx, 5
    mov eax, 89
    syscall
    cmp rax, 5
    jne failure
    cmp dword ptr [0x401040], 0x696e692f
    jne failure
    cmp byte ptr [0x401044], 't'
    jne failure

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

    mov edi, 0x402200
    mov eax, 63
    syscall
    cmp rax, -14
    jne failure

    mov edi, 0x401000
    mov eax, 63
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [rdi], 0x756e6947
    jne failure

    mov eax, 102
    syscall
    test rax, rax
    jne failure
    mov eax, 104
    syscall
    test rax, rax
    jne failure
    mov eax, 107
    syscall
    test rax, rax
    jne failure
    mov eax, 108
    syscall
    test rax, rax
    jne failure

    mov dword ptr [0x401060], 0x12345678
    mov dword ptr [0x401064], 0x12345678
    mov edi, 0x401060
    mov esi, 0x401064
    mov edx, 0x402200
    mov eax, 118
    syscall
    cmp rax, -14
    jne failure
    cmp dword ptr [0x401060], 0x12345678
    jne failure
    cmp dword ptr [0x401064], 0x12345678
    jne failure

    mov edi, 0x401060
    mov esi, 0x401064
    mov edx, 0x401068
    mov eax, 118
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x401060], 0
    jne failure
    cmp dword ptr [0x401064], 0
    jne failure
    cmp dword ptr [0x401068], 0
    jne failure

    mov dword ptr [0x401060], 0x12345678
    mov dword ptr [0x401064], 0x12345678
    mov dword ptr [0x401068], 0x12345678
    mov edi, 0x401060
    mov esi, 0x401064
    mov edx, 0x401068
    mov eax, 120
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x401060], 0
    jne failure
    cmp dword ptr [0x401064], 0
    jne failure
    cmp dword ptr [0x401068], 0
    jne failure

    mov eax, 186
    syscall
    cmp rax, 1
    jne failure

    mov eax, 110
    syscall
    test rax, rax
    jne failure

    mov eax, 111
    syscall
    cmp rax, 1
    jne failure

    xor edi, edi
    mov eax, 121
    syscall
    cmp rax, 1
    jne failure
    mov edi, 2
    mov eax, 121
    syscall
    cmp rax, -3
    jne failure

    xor edi, edi
    mov eax, 124
    syscall
    cmp rax, 1
    jne failure
    mov edi, 2
    mov eax, 124
    syscall
    cmp rax, -3
    jne failure

    mov edi, 0x402200
    mov eax, 218
    syscall
    cmp rax, -14
    jne failure

    mov dword ptr [0x401190], 0x12345678
    mov edi, 0x401190
    mov eax, 218
    syscall
    cmp rax, 1
    jne failure

    mov r10d, 0x12345678
    mov edi, 0x11223344
    mov eax, 39
    syscall
    cmp rax, 1
    jne failure
    cmp r10d, 0x12345678
    jne failure
    cmp edi, 0x11223344
    jne failure

    mov edi, 42
    mov eax, 231
    syscall
failure:
    ud2

.balign 8
readable_payload:
    .byte 1, 2, 3, 4, 5, 6, 7, 8
proc_self_exe:
    .asciz "/proc/self/exe"

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
    syscall
    cmp rax, 8
    jne failure

    mov edi, 0x400000
    mov eax, 63
    syscall
    cmp rax, -14
    jne failure

    movabs rax, 0x0a212145424f5250
    mov qword ptr [rsp - 16], rax
    mov edi, 1
    lea rsi, [rsp - 16]
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure
    mov edi, 1
    mov esi, 0x406ff9
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, -14
    jne failure

    xor edi, edi
    mov eax, 12
    syscall
    cmp rax, 0x404000
    jne failure
    mov edi, 0x404080
    mov eax, 12
    syscall
    cmp rax, 0x404080
    jne failure
    movabs rax, 0x0a212145424f5250
    mov qword ptr [0x404000], rax
    mov edi, 1
    mov esi, 0x404000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure
    mov edi, 1
    mov esi, 0x404079
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x4845415042524b21
    mov qword ptr [0x404000], rax
    mov edi, 0x405001
    mov eax, 12
    syscall
    cmp rax, 0x404080
    jne failure
    mov edi, 0x404040
    mov eax, 12
    syscall
    cmp rax, 0x404040
    jne failure

    mov eax, 999
    syscall
    cmp rax, -38
    jne failure

    mov edi, 2
    mov esi, 0x402000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, -9
    jne failure

    mov edi, 1
    mov esi, 0x402000
    mov edx, 257
    mov eax, 1
    syscall
    cmp rax, -22
    jne failure

    mov edi, 1
    mov esi, 0x402000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure

    mov edi, 0x1002
    mov esi, 0x402000
    mov eax, 158
    syscall
    test rax, rax
    jne failure
    mov rbx, qword ptr fs:[0]
    movabs rax, 0x0a212145424f5250
    cmp rbx, rax
    jne failure

    mov edi, 0x1003
    mov esi, 0x403200
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
    mov esi, 0x402008
    mov eax, 158
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x402008], 0x402000
    jne failure

    mov edi, 0x402020
    mov esi, 1
    mov eax, 79
    syscall
    cmp rax, -34
    jne failure
    mov edi, 0x403200
    mov esi, 2
    mov eax, 79
    syscall
    cmp rax, -14
    jne failure
    mov edi, 0x402020
    mov esi, 2
    mov eax, 79
    syscall
    cmp rax, 2
    jne failure
    cmp word ptr [0x402020], 0x002f
    jne failure

    lea rdi, [rip + proc_self_exe]
    mov esi, 0x402040
    xor edx, edx
    mov eax, 89
    syscall
    cmp rax, -22
    jne failure
    mov edi, 0x407000
    mov esi, 0x402040
    mov edx, 5
    mov eax, 89
    syscall
    cmp rax, -14
    jne failure
    lea rdi, [rip + proc_self_exe]
    mov esi, 0x403200
    mov edx, 5
    mov eax, 89
    syscall
    cmp rax, -14
    jne failure
    lea rdi, [rip + proc_self_exe]
    mov esi, 0x402040
    mov edx, 5
    mov eax, 89
    syscall
    cmp rax, 5
    jne failure
    cmp dword ptr [0x402040], 0x696e692f
    jne failure
    cmp byte ptr [0x402044], 't'
    jne failure

    mov edi, -100
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x402040
    mov r10d, 5
    mov eax, 267
    syscall
    cmp rax, 5
    jne failure
    cmp dword ptr [0x402040], 0x696e692f
    jne failure
    cmp byte ptr [0x402044], 't'
    jne failure

    mov edi, -99
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x402040
    mov r10d, 5
    mov eax, 267
    syscall
    cmp rax, -9
    jne failure

    mov edi, -100
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x402040
    xor r10d, r10d
    mov eax, 267
    syscall
    cmp rax, -22
    jne failure

    mov edi, -100
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x403200
    mov r10d, 5
    mov eax, 267
    syscall
    cmp rax, -14
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

    mov edi, 0x403200
    mov eax, 63
    syscall
    cmp rax, -14
    jne failure

    mov edi, 0x402000
    mov eax, 63
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [rdi], 0x756e6947
    jne failure

    mov edi, 2
    mov esi, 0x402200
    mov eax, 5
    syscall
    cmp rax, -9
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4031c0], rax
    mov edi, 1
    mov esi, 0x4031c0
    mov eax, 5
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4031c0], rax
    jne failure

    mov edi, 1
    mov esi, 0x402200
    mov eax, 5
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x402210], 1
    jne failure
    cmp dword ptr [0x402218], 0x2180
    jne failure
    cmp qword ptr [0x402238], 4096
    jne failure

    mov edi, 2
    mov esi, 0x5413
    mov edx, 0x402280
    mov eax, 16
    syscall
    cmp rax, -9
    jne failure

    mov edi, 1
    mov esi, 0x5402
    mov edx, 0x402280
    mov eax, 16
    syscall
    cmp rax, -25
    jne failure

    mov edi, 1
    mov esi, 0x5413
    mov edx, 0x4031fc
    mov eax, 16
    syscall
    cmp rax, -14
    jne failure

    mov edi, 1
    mov esi, 0x5413
    mov edx, 0x402280
    mov eax, 16
    syscall
    test rax, rax
    jne failure
    cmp word ptr [0x402280], 24
    jne failure
    cmp word ptr [0x402282], 80
    jne failure
    cmp dword ptr [0x402284], 0
    jne failure

    mov edi, 2
    movabs rsi, 0x1122334455667788
    mov edx, 2
    mov eax, 8
    syscall
    cmp rax, -9
    jne failure

    mov edi, 1
    movabs rsi, 0x1122334455667788
    mov edx, 99
    mov eax, 8
    syscall
    cmp rax, -29
    jne failure

    lea rax, [rip + readable_payload]
    mov qword ptr [0x402300], rax
    mov qword ptr [0x402308], 4
    mov qword ptr [0x402310], 0x403200
    mov qword ptr [0x402318], 4
    mov edi, 1
    mov esi, 0x402300
    mov edx, 2
    mov eax, 20
    syscall
    cmp rax, -14
    jne failure

    lea rax, [rip + readable_payload + 4]
    mov qword ptr [0x402310], rax
    mov edi, 2
    mov esi, 0x402300
    mov edx, 2
    mov eax, 20
    syscall
    cmp rax, -9
    jne failure

    mov edi, 1
    mov esi, 0x402300
    mov edx, 5
    mov eax, 20
    syscall
    cmp rax, -22
    jne failure

    mov edi, 1
    mov esi, 0x402300
    mov edx, 2
    mov eax, 20
    syscall
    cmp rax, 8
    jne failure

    mov edi, 99
    mov esi, 0x402080
    mov eax, 97
    syscall
    cmp rax, -22
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4031f8], rax
    mov edi, 3
    mov esi, 0x4031f8
    mov eax, 97
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4031f8], rax
    jne failure

    mov edi, 3
    mov esi, 0x402080
    mov eax, 97
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x402080], 0x1000
    jne failure
    cmp qword ptr [0x402088], 0x1000
    jne failure

    mov edi, 2
    mov esi, 7
    xor edx, edx
    mov r10d, 0x402090
    mov eax, 302
    syscall
    cmp rax, -3
    jne failure

    xor edi, edi
    mov esi, 99
    xor edx, edx
    mov r10d, 0x402090
    mov eax, 302
    syscall
    cmp rax, -22
    jne failure

    xor edi, edi
    mov esi, 7
    mov edx, 0x402080
    xor r10d, r10d
    mov eax, 302
    syscall
    cmp rax, -1
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4031f8], rax
    xor edi, edi
    mov esi, 7
    xor edx, edx
    mov r10d, 0x4031f8
    mov eax, 302
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4031f8], rax
    jne failure

    xor edi, edi
    mov esi, 7
    xor edx, edx
    mov r10d, 0x402090
    mov eax, 302
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x402090], 2
    jne failure
    cmp qword ptr [0x402098], 2
    jne failure

    mov edi, 2
    mov esi, 8
    mov edx, 0x4020a0
    mov eax, 204
    syscall
    cmp rax, -3
    jne failure

    xor edi, edi
    mov esi, 7
    mov edx, 0x4020a0
    mov eax, 204
    syscall
    cmp rax, -22
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4031fc], rax
    xor edi, edi
    mov esi, 8
    mov edx, 0x4031fc
    mov eax, 204
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4031fc], rax
    jne failure

    xor edi, edi
    mov esi, 8
    mov edx, 0x4020a0
    mov eax, 204
    syscall
    cmp rax, 8
    jne failure
    cmp qword ptr [0x4020a0], 1
    jne failure

    mov dword ptr [0x4020a8], 0x12345678
    mov edi, 0x4020a8
    mov esi, 0x403200
    mov edx, 0xdeadbeef
    mov eax, 309
    syscall
    cmp rax, -14
    jne failure
    cmp dword ptr [0x4020a8], 0x12345678
    jne failure

    mov edi, 0x4020a8
    mov esi, 0x4020ac
    mov edx, 0xdeadbeef
    mov eax, 309
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x4020a8], 0
    jne failure
    cmp dword ptr [0x4020ac], 0
    jne failure

    mov edi, 1
    mov esi, 0x403200
    mov rdx, -1
    xor eax, eax
    syscall
    cmp rax, -9
    jne failure

    xor edi, edi
    mov esi, 0x403200
    mov rdx, -1
    xor eax, eax
    syscall
    test rax, rax
    jne failure

    mov edi, 2
    mov eax, 3
    syscall
    cmp rax, -9
    jne failure

    xor edi, edi
    mov eax, 3
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov eax, 3
    syscall
    cmp rax, -9
    jne failure

    xor edi, edi
    mov esi, 0x402200
    mov eax, 5
    syscall
    cmp rax, -9
    jne failure

    xor edi, edi
    mov esi, 0x5413
    mov edx, 0x402280
    mov eax, 16
    syscall
    cmp rax, -9
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

    mov dword ptr [0x402060], 0x12345678
    mov dword ptr [0x402064], 0x12345678
    mov edi, 0x402060
    mov esi, 0x402064
    mov edx, 0x403200
    mov eax, 118
    syscall
    cmp rax, -14
    jne failure
    cmp dword ptr [0x402060], 0x12345678
    jne failure
    cmp dword ptr [0x402064], 0x12345678
    jne failure

    mov edi, 0x402060
    mov esi, 0x402064
    mov edx, 0x402068
    mov eax, 118
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x402060], 0
    jne failure
    cmp dword ptr [0x402064], 0
    jne failure
    cmp dword ptr [0x402068], 0
    jne failure

    mov dword ptr [0x402060], 0x12345678
    mov dword ptr [0x402064], 0x12345678
    mov dword ptr [0x402068], 0x12345678
    mov edi, 0x402060
    mov esi, 0x402064
    mov edx, 0x402068
    mov eax, 120
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x402060], 0
    jne failure
    cmp dword ptr [0x402064], 0
    jne failure
    cmp dword ptr [0x402068], 0
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

    mov edi, 0x4021b0
    mov esi, 23
    mov eax, 273
    syscall
    cmp rax, -22
    jne failure

    mov edi, 0x4031f0
    mov esi, 24
    mov eax, 273
    syscall
    cmp rax, -14
    jne failure

    mov edi, 0x4021b0
    mov esi, 24
    mov eax, 273
    syscall
    test rax, rax
    jne failure

    mov edi, 2
    mov esi, 0x4021c8
    mov edx, 0x4021d0
    mov eax, 274
    syscall
    cmp rax, -3
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4021c8], rax
    xor edi, edi
    mov esi, 0x4021c8
    mov edx, 0x4031fc
    mov eax, 274
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4021c8], rax
    jne failure

    xor edi, edi
    mov esi, 0x4021c8
    mov edx, 0x4021d0
    mov eax, 274
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4021c8], 0x4021b0
    jne failure
    cmp qword ptr [0x4021d0], 24
    jne failure

    mov edi, 0x403001
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    syscall
    cmp rax, -22
    jne failure

    mov edi, 0x403000
    mov esi, 4096
    mov edx, 4
    mov eax, 10
    syscall
    cmp rax, -22
    jne failure

    movabs rax, 0x0a212145424f5250
    mov qword ptr [0x403000], rax
    mov edi, 0x403000
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    syscall
    test rax, rax
    jne failure

    mov edi, 0x403000
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    syscall
    cmp rax, -22
    jne failure

    mov edi, 0x403000
    mov eax, 63
    syscall
    cmp rax, -14
    jne failure

    mov edi, 1
    mov esi, 0x403000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x402280], rax
    mov qword ptr [0x402298], rax
    mov edi, 1
    mov esi, 0x5401
    mov edx, 0x402280
    mov eax, 16
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x402280], 0
    jne failure
    cmp qword ptr [0x402298], 0
    jne failure

    mov edi, 2
    mov eax, 32
    syscall
    cmp rax, -9
    jne failure

    mov edi, 2
    xor esi, esi
    xor edx, edx
    mov eax, 72
    syscall
    cmp rax, -9
    jne failure

    mov edi, 1
    xor esi, esi
    mov edx, 2
    mov eax, 72
    syscall
    cmp rax, -22
    jne failure

    mov edi, 1
    xor esi, esi
    xor edx, edx
    mov eax, 72
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov esi, 2
    mov edx, 1
    mov eax, 72
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov esi, 1
    mov eax, 72
    syscall
    cmp rax, 1
    jne failure

    xor edi, edi
    mov esi, 2
    mov edx, 2
    mov eax, 72
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov esi, 1
    mov eax, 72
    syscall
    test rax, rax
    jne failure

    mov edi, 1
    xor esi, esi
    xor edx, edx
    mov eax, 72
    syscall
    cmp rax, -24
    jne failure

    xor edi, edi
    mov eax, 3
    syscall
    test rax, rax
    jne failure

    mov edi, 1
    mov esi, 1030
    xor edx, edx
    mov eax, 72
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov esi, 1
    mov eax, 72
    syscall
    cmp rax, 1
    jne failure

    xor edi, edi
    mov eax, 3
    syscall
    test rax, rax
    jne failure

    mov edi, 1
    mov eax, 32
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov esi, 1
    mov eax, 72
    syscall
    test rax, rax
    jne failure

    mov edi, 1
    mov eax, 32
    syscall
    cmp rax, -24
    jne failure

    xor edi, edi
    lea rsi, [rip + readable_payload]
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure

    xor edi, edi
    xor esi, esi
    mov eax, 33
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov esi, 1
    mov eax, 33
    syscall
    cmp rax, 1
    jne failure

    xor edi, edi
    mov esi, 2
    mov eax, 33
    syscall
    cmp rax, -9
    jne failure

    mov edi, 2
    mov esi, 1
    mov eax, 33
    syscall
    cmp rax, -9
    jne failure

    xor edi, edi
    mov esi, 3
    mov eax, 72
    syscall
    cmp rax, 1
    jne failure

    mov edi, 1
    mov esi, 1
    mov eax, 72
    syscall
    test rax, rax
    jne failure

    mov edi, 1
    mov eax, 3
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov eax, 3
    syscall
    test rax, rax
    jne failure

    mov edi, 0x403200
    mov eax, 218
    syscall
    cmp rax, -14
    jne failure

    mov dword ptr [0x402190], 0x12345678
    mov edi, 0x402190
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
    .ascii "PROBE!!\n"
proc_self_exe:
    .asciz "/proc/self/exe"

.section .data
.balign 8
.global ginux_linux_probe_data
ginux_linux_probe_data:
    .ascii "PROBE!!\n"

.section .bss
.balign 8
    .space 4088

.section .data.rel.ro,"aw",@progbits
.balign 8
    .space 512

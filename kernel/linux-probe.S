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

    mov eax, 24
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov eax, 145
    syscall
    test rax, rax
    jne failure
    mov edi, 2
    mov eax, 145
    syscall
    cmp rax, -3
    jne failure

    xor edi, edi
    mov esi, 0x403200
    mov eax, 143
    syscall
    cmp rax, -14
    jne failure
    mov dword ptr [0x402060], 0x12345678
    mov esi, 0x402060
    mov eax, 143
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x402060], 0
    jne failure

    xor edi, edi
    mov eax, 146
    syscall
    test rax, rax
    jne failure
    mov eax, 147
    syscall
    test rax, rax
    jne failure
    mov edi, 1
    mov eax, 146
    syscall
    cmp rax, -22
    jne failure

    xor edi, edi
    xor esi, esi
    mov eax, 140
    syscall
    cmp rax, 20
    jne failure
    mov esi, 2
    mov eax, 140
    syscall
    cmp rax, -3
    jne failure
    mov edi, 1
    xor esi, esi
    mov eax, 140
    syscall
    cmp rax, -22
    jne failure
    xor edi, edi
    xor esi, esi
    xor edx, edx
    mov eax, 141
    syscall
    test rax, rax
    jne failure
    mov edx, 1
    mov eax, 141
    syscall
    cmp rax, -1
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

    mov qword ptr [0x4020c0], 0x40102
    mov edi, 99
    xor esi, esi
    mov edx, 0x4020e0
    mov r10d, 7
    mov eax, 14
    syscall
    cmp rax, -22
    jne failure

    mov edi, 99
    xor esi, esi
    mov edx, 0x4020e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4020e0], 0
    jne failure

    xor edi, edi
    mov esi, 0x4020c0
    mov edx, 0x4020e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4020e0], 0
    jne failure

    mov edi, 99
    xor esi, esi
    mov edx, 0x4020e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4020e0], 2
    jne failure

    mov qword ptr [0x4020c0], 8
    xor edi, edi
    mov esi, 0x4020c0
    mov edx, 0x403200
    mov r10d, 8
    mov eax, 14
    syscall
    cmp rax, -14
    jne failure

    mov edi, 99
    xor esi, esi
    mov edx, 0x4020e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4020e0], 2
    jne failure

    mov qword ptr [0x4020c0], 2
    mov edi, 1
    mov esi, 0x4020c0
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure

    mov edi, 99
    xor esi, esi
    mov edx, 0x4020e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4020e0], 0
    jne failure

    xor edi, edi
    mov esi, 0x403200
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    cmp rax, -14
    jne failure

    mov edi, 10
    xor esi, esi
    xor edx, edx
    mov r10d, 7
    mov eax, 13
    syscall
    cmp rax, -22
    jne failure

    xor edi, edi
    xor esi, esi
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    cmp rax, -22
    jne failure

    mov qword ptr [0x402100], 1
    mov qword ptr [0x402108], 0x1234
    mov qword ptr [0x402110], 0x400100
    mov qword ptr [0x402118], 0x40102
    mov edi, 9
    mov esi, 0x402100
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    cmp rax, -22
    jne failure

    mov edi, 10
    mov esi, 0x403200
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    cmp rax, -14
    jne failure

    mov edi, 10
    mov esi, 0x402100
    mov edx, 0x402120
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x402120], 0
    jne failure
    cmp qword ptr [0x402138], 0
    jne failure

    mov edi, 10
    xor esi, esi
    mov edx, 0x402120
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x402120], 1
    jne failure
    cmp qword ptr [0x402128], 0x1234
    jne failure
    cmp qword ptr [0x402130], 0x400100
    jne failure
    cmp qword ptr [0x402138], 2
    jne failure

    mov qword ptr [0x402100], 0
    mov qword ptr [0x402108], 0
    mov qword ptr [0x402110], 0
    mov qword ptr [0x402118], 0
    mov edi, 10
    mov esi, 0x402100
    mov edx, 0x403200
    mov r10d, 8
    mov eax, 13
    syscall
    cmp rax, -14
    jne failure

    mov edi, 10
    xor esi, esi
    mov edx, 0x402120
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x402120], 1
    jne failure

    mov edi, 10
    mov esi, 0x402100
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov esi, 0x402158
    mov eax, 131
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x402158], 0
    jne failure
    cmp dword ptr [0x402160], 2
    jne failure
    cmp qword ptr [0x402168], 0
    jne failure

    xor edi, edi
    mov esi, 0x403200
    mov eax, 131
    syscall
    cmp rax, -14
    jne failure

    mov qword ptr [0x402140], 0x402800
    mov qword ptr [0x402148], 0
    mov qword ptr [0x402150], 2048
    mov edi, 0x402140
    mov esi, 0x402158
    mov eax, 131
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x402160], 2
    jne failure

    xor edi, edi
    mov esi, 0x402158
    mov eax, 131
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x402158], 0x402800
    jne failure
    cmp dword ptr [0x402160], 0
    jne failure
    cmp qword ptr [0x402168], 2048
    jne failure

    mov qword ptr [0x402140], 0x403000
    mov qword ptr [0x402148], 0
    mov qword ptr [0x402150], 2048
    mov edi, 0x402140
    xor esi, esi
    mov eax, 131
    syscall
    cmp rax, -14
    jne failure

    mov qword ptr [0x402140], 0x402800
    mov qword ptr [0x402148], 0
    mov qword ptr [0x402150], 1024
    mov edi, 0x402140
    xor esi, esi
    mov eax, 131
    syscall
    cmp rax, -12
    jne failure

    mov qword ptr [0x402140], 0
    mov qword ptr [0x402148], 2
    mov qword ptr [0x402150], 0
    mov edi, 0x402140
    xor esi, esi
    mov eax, 131
    syscall
    test rax, rax
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

    mov edi, 0x403200
    mov eax, 80
    syscall
    cmp rax, -14
    jne failure

    mov word ptr [0x4020b0], 0x2f
    mov edi, 0x4020b0
    mov eax, 80
    syscall
    test rax, rax
    jne failure

    movabs rax, 0x676e697373696d2f
    mov qword ptr [0x4020b0], rax
    mov byte ptr [0x4020b8], 0
    mov edi, 0x4020b0
    mov eax, 80
    syscall
    cmp rax, -2
    jne failure

    mov edi, 16
    mov esi, 0x4020d0
    mov eax, 157
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x4020d0], 0x74696e69
    jne failure
    cmp dword ptr [0x4020d4], 0
    jne failure

    mov edi, 15
    mov esi, 0x403200
    mov eax, 157
    syscall
    cmp rax, -14
    jne failure

    movabs rax, 0x72702d78756e6967
    mov qword ptr [0x4020c0], rax
    mov dword ptr [0x4020c8], 0x0065626f
    mov edi, 15
    mov esi, 0x4020c0
    mov eax, 157
    syscall
    test rax, rax
    jne failure

    mov edi, 16
    mov esi, 0x403200
    mov eax, 157
    syscall
    cmp rax, -14
    jne failure

    mov edi, 16
    mov esi, 0x4020d0
    mov eax, 157
    syscall
    test rax, rax
    jne failure
    movabs rax, 0x72702d78756e6967
    cmp qword ptr [0x4020d0], rax
    jne failure
    cmp dword ptr [0x4020d8], 0x0065626f
    jne failure
    cmp dword ptr [0x4020dc], 0
    jne failure

    mov edi, 99
    mov eax, 157
    syscall
    cmp rax, -22
    jne failure

    mov dword ptr [0x4020c0], 0x74696e69
    mov byte ptr [0x4020c4], 0
    mov edi, 15
    mov esi, 0x4020c0
    mov eax, 157
    syscall
    test rax, rax
    jne failure

    mov edi, 39
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    syscall
    test rax, rax
    jne failure

    mov edi, 38
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    syscall
    cmp rax, -22
    jne failure

    mov edi, 38
    mov esi, 1
    mov edx, 1
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    syscall
    cmp rax, -22
    jne failure

    mov edi, 38
    mov esi, 1
    xor edx, edx
    mov r10d, 1
    xor r8d, r8d
    mov eax, 157
    syscall
    cmp rax, -22
    jne failure

    mov edi, 39
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    syscall
    test rax, rax
    jne failure

    mov edi, 38
    mov esi, 1
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    syscall
    test rax, rax
    jne failure

    mov edi, 39
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    syscall
    cmp rax, 1
    jne failure

    mov edi, 38
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    syscall
    cmp rax, -22
    jne failure

    mov edi, 39
    mov esi, 1
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 157
    syscall
    cmp rax, -22
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

    mov rdi, -1
    xor esi, esi
    mov eax, 115
    syscall
    cmp rax, -22
    jne failure

    xor edi, edi
    mov esi, 0x403200
    mov eax, 115
    syscall
    cmp rax, 1
    jne failure

    mov dword ptr [0x402060], 0x12345678
    mov edi, 1
    mov esi, 0x403200
    mov eax, 115
    syscall
    cmp rax, -14
    jne failure
    cmp dword ptr [0x402060], 0x12345678
    jne failure

    mov edi, 1
    mov esi, 0x402060
    mov eax, 115
    syscall
    cmp rax, 1
    jne failure
    cmp dword ptr [0x402060], 0
    jne failure

    mov edi, -1
    mov eax, 135
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov eax, 135
    syscall
    test rax, rax
    jne failure

    mov edi, 0x40000
    mov eax, 135
    syscall
    cmp rax, -22
    jne failure

    mov dword ptr [0x402060], 0x20080522
    mov dword ptr [0x402064], 0
    mov dword ptr [0x402068], 0x12345678
    mov edi, 0x402060
    mov esi, 0x403200
    mov eax, 125
    syscall
    cmp rax, -14
    jne failure
    cmp dword ptr [0x402068], 0x12345678
    jne failure

    mov edi, 0x402060
    mov esi, 0x402068
    mov eax, 125
    syscall
    test rax, rax
    jne failure
    mov eax, dword ptr [0x402068]
    or eax, dword ptr [0x40206c]
    or eax, dword ptr [0x402070]
    or eax, dword ptr [0x402074]
    or eax, dword ptr [0x402078]
    or eax, dword ptr [0x40207c]
    jne failure

    mov dword ptr [0x402060], 0
    mov edi, 0x402060
    xor esi, esi
    mov eax, 125
    syscall
    cmp rax, -22
    jne failure
    cmp dword ptr [0x402060], 0x20080522
    jne failure

    mov dword ptr [0x402064], 2
    mov eax, 125
    syscall
    cmp rax, -3
    jne failure

    mov dword ptr [0x402064], 0
    mov edi, 0x402060
    mov esi, 0x402068
    mov eax, 126
    syscall
    test rax, rax
    jne failure
    mov dword ptr [0x402068], 1
    mov eax, 126
    syscall
    cmp rax, -1
    jne failure
    mov dword ptr [0x402068], 0

    xor edi, edi
    xor esi, esi
    mov eax, 116
    syscall
    cmp rax, -1
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

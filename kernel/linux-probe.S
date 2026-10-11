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
    mov esi, 0x407ff9
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, -14
    jne failure

    xor edi, edi
    mov eax, 12
    syscall
    cmp rax, 0x405000
    jne failure
    mov edi, 0x405080
    mov eax, 12
    syscall
    cmp rax, 0x405080
    jne failure
    movabs rax, 0x0a212145424f5250
    mov qword ptr [0x405000], rax
    mov edi, 1
    mov esi, 0x405000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure
    mov edi, 1
    mov esi, 0x405079
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x4845415042524b21
    mov qword ptr [0x405000], rax
    mov edi, 0x406001
    mov eax, 12
    syscall
    cmp rax, 0x405080
    jne failure
    mov edi, 0x405040
    mov eax, 12
    syscall
    cmp rax, 0x405040
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
    mov esi, 0x404200
    mov eax, 143
    syscall
    cmp rax, -14
    jne failure
    mov dword ptr [0x403060], 0x12345678
    mov esi, 0x403060
    mov eax, 143
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x403060], 0
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
    mov esi, 0x404200
    mov eax, 142
    syscall
    cmp rax, -14
    jne failure
    mov dword ptr [0x403060], 0
    mov esi, 0x403060
    mov eax, 142
    syscall
    test rax, rax
    jne failure
    xor esi, esi
    mov edx, 0x403060
    mov eax, 144
    syscall
    test rax, rax
    jne failure
    mov esi, 1
    mov eax, 144
    syscall
    cmp rax, -1
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
    mov esi, 0x403000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, -9
    jne failure

    mov edi, 1
    mov esi, 0x403000
    mov edx, 257
    mov eax, 1
    syscall
    cmp rax, -22
    jne failure

    mov qword ptr [0x4030c0], 0x40102
    mov edi, 99
    xor esi, esi
    mov edx, 0x4030e0
    mov r10d, 7
    mov eax, 14
    syscall
    cmp rax, -22
    jne failure

    mov edi, 99
    xor esi, esi
    mov edx, 0x4030e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4030e0], 0
    jne failure

    xor edi, edi
    mov esi, 0x4030c0
    mov edx, 0x4030e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4030e0], 0
    jne failure

    mov edi, 99
    xor esi, esi
    mov edx, 0x4030e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4030e0], 2
    jne failure

    mov qword ptr [0x4030c0], 8
    xor edi, edi
    mov esi, 0x4030c0
    mov edx, 0x404200
    mov r10d, 8
    mov eax, 14
    syscall
    cmp rax, -14
    jne failure

    mov edi, 99
    xor esi, esi
    mov edx, 0x4030e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4030e0], 2
    jne failure

    mov qword ptr [0x4030c0], 2
    mov edi, 1
    mov esi, 0x4030c0
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure

    mov edi, 99
    xor esi, esi
    mov edx, 0x4030e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4030e0], 0
    jne failure

    xor edi, edi
    mov esi, 0x404200
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

    mov qword ptr [0x403100], 1
    mov qword ptr [0x403108], 0x1234
    mov qword ptr [0x403110], 0x400100
    mov qword ptr [0x403118], 0x40102
    mov edi, 9
    mov esi, 0x403100
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    cmp rax, -22
    jne failure

    mov edi, 10
    mov esi, 0x404200
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    cmp rax, -14
    jne failure

    mov edi, 10
    mov esi, 0x403100
    mov edx, 0x403120
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x403120], 0
    jne failure
    cmp qword ptr [0x403138], 0
    jne failure

    mov edi, 10
    xor esi, esi
    mov edx, 0x403120
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x403120], 1
    jne failure
    cmp qword ptr [0x403128], 0x1234
    jne failure
    cmp qword ptr [0x403130], 0x400100
    jne failure
    cmp qword ptr [0x403138], 2
    jne failure

    mov qword ptr [0x403100], 0
    mov qword ptr [0x403108], 0
    mov qword ptr [0x403110], 0
    mov qword ptr [0x403118], 0
    mov edi, 10
    mov esi, 0x403100
    mov edx, 0x404200
    mov r10d, 8
    mov eax, 13
    syscall
    cmp rax, -14
    jne failure

    mov edi, 10
    xor esi, esi
    mov edx, 0x403120
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x403120], 1
    jne failure

    mov edi, 10
    mov esi, 0x403100
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov esi, 0x403158
    mov eax, 131
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x403158], 0
    jne failure
    cmp dword ptr [0x403160], 2
    jne failure
    cmp qword ptr [0x403168], 0
    jne failure

    xor edi, edi
    mov esi, 0x404200
    mov eax, 131
    syscall
    cmp rax, -14
    jne failure

    mov qword ptr [0x403140], 0x403800
    mov qword ptr [0x403148], 0
    mov qword ptr [0x403150], 2048
    mov edi, 0x403140
    mov esi, 0x403158
    mov eax, 131
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x403160], 2
    jne failure

    xor edi, edi
    mov esi, 0x403158
    mov eax, 131
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x403158], 0x403800
    jne failure
    cmp dword ptr [0x403160], 0
    jne failure
    cmp qword ptr [0x403168], 2048
    jne failure

    mov qword ptr [0x403140], 0x404000
    mov qword ptr [0x403148], 0
    mov qword ptr [0x403150], 2048
    mov edi, 0x403140
    xor esi, esi
    mov eax, 131
    syscall
    cmp rax, -14
    jne failure

    mov qword ptr [0x403140], 0x403800
    mov qword ptr [0x403148], 0
    mov qword ptr [0x403150], 1024
    mov edi, 0x403140
    xor esi, esi
    mov eax, 131
    syscall
    cmp rax, -12
    jne failure

    mov qword ptr [0x403140], 0
    mov qword ptr [0x403148], 2
    mov qword ptr [0x403150], 0
    mov edi, 0x403140
    xor esi, esi
    mov eax, 131
    syscall
    test rax, rax
    jne failure

    mov edi, 1
    mov esi, 0x403000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure

    mov edi, 0x1002
    mov esi, 0x403000
    mov eax, 158
    syscall
    test rax, rax
    jne failure
    mov rbx, qword ptr fs:[0]
    movabs rax, 0x0a212145424f5250
    cmp rbx, rax
    jne failure

    mov edi, 0x1003
    mov esi, 0x404200
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
    mov esi, 0x403008
    mov eax, 158
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x403008], 0x403000
    jne failure

    mov edi, 0x403020
    mov esi, 1
    mov eax, 79
    syscall
    cmp rax, -34
    jne failure
    mov edi, 0x404200
    mov esi, 2
    mov eax, 79
    syscall
    cmp rax, -14
    jne failure
    mov edi, 0x403020
    mov esi, 2
    mov eax, 79
    syscall
    cmp rax, 2
    jne failure
    cmp word ptr [0x403020], 0x002f
    jne failure

    lea rdi, [rip + proc_self_exe]
    mov esi, 0x403040
    xor edx, edx
    mov eax, 89
    syscall
    cmp rax, -22
    jne failure
    mov edi, 0x408000
    mov esi, 0x403040
    mov edx, 5
    mov eax, 89
    syscall
    cmp rax, -14
    jne failure
    lea rdi, [rip + proc_self_exe]
    mov esi, 0x404200
    mov edx, 5
    mov eax, 89
    syscall
    cmp rax, -14
    jne failure
    lea rdi, [rip + proc_self_exe]
    mov esi, 0x403040
    mov edx, 5
    mov eax, 89
    syscall
    cmp rax, 5
    jne failure
    cmp dword ptr [0x403040], 0x696e692f
    jne failure
    cmp byte ptr [0x403044], 't'
    jne failure

    mov edi, -100
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x403040
    mov r10d, 5
    mov eax, 267
    syscall
    cmp rax, 5
    jne failure
    cmp dword ptr [0x403040], 0x696e692f
    jne failure
    cmp byte ptr [0x403044], 't'
    jne failure

    mov edi, -99
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x403040
    mov r10d, 5
    mov eax, 267
    syscall
    cmp rax, -9
    jne failure

    mov edi, -100
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x403040
    xor r10d, r10d
    mov eax, 267
    syscall
    cmp rax, -22
    jne failure

    mov edi, -100
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x404200
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

    mov edi, 0x404200
    mov eax, 63
    syscall
    cmp rax, -14
    jne failure

    mov edi, 0x403000
    mov eax, 63
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [rdi], 0x756e6947
    jne failure

    mov edi, 2
    mov esi, 0x403200
    mov eax, 5
    syscall
    cmp rax, -9
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4041c0], rax
    mov edi, 1
    mov esi, 0x4041c0
    mov eax, 5
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4041c0], rax
    jne failure

    mov edi, 1
    mov esi, 0x403200
    mov eax, 5
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x403210], 1
    jne failure
    cmp dword ptr [0x403218], 0x2180
    jne failure
    cmp qword ptr [0x403238], 4096
    jne failure

    mov edi, 2
    mov esi, 0x5413
    mov edx, 0x403280
    mov eax, 16
    syscall
    cmp rax, -9
    jne failure

    mov edi, 1
    mov esi, 0x5402
    mov edx, 0x403280
    mov eax, 16
    syscall
    cmp rax, -25
    jne failure

    mov edi, 1
    mov esi, 0x5413
    mov edx, 0x4041fc
    mov eax, 16
    syscall
    cmp rax, -14
    jne failure

    mov edi, 1
    mov esi, 0x5413
    mov edx, 0x403280
    mov eax, 16
    syscall
    test rax, rax
    jne failure
    cmp word ptr [0x403280], 24
    jne failure
    cmp word ptr [0x403282], 80
    jne failure
    cmp dword ptr [0x403284], 0
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
    mov qword ptr [0x403300], rax
    mov qword ptr [0x403308], 4
    mov qword ptr [0x403310], 0x404200
    mov qword ptr [0x403318], 4
    mov edi, 1
    mov esi, 0x403300
    mov edx, 2
    mov eax, 20
    syscall
    cmp rax, -14
    jne failure

    lea rax, [rip + readable_payload + 4]
    mov qword ptr [0x403310], rax
    mov edi, 2
    mov esi, 0x403300
    mov edx, 2
    mov eax, 20
    syscall
    cmp rax, -9
    jne failure

    mov edi, 1
    mov esi, 0x403300
    mov edx, 5
    mov eax, 20
    syscall
    cmp rax, -22
    jne failure

    mov edi, 1
    mov esi, 0x403300
    mov edx, 2
    mov eax, 20
    syscall
    cmp rax, 8
    jne failure

    mov edi, 99
    mov esi, 0x403080
    mov eax, 97
    syscall
    cmp rax, -22
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4041f8], rax
    mov edi, 3
    mov esi, 0x4041f8
    mov eax, 97
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4041f8], rax
    jne failure

    mov edi, 3
    mov esi, 0x403080
    mov eax, 97
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x403080], 0x1000
    jne failure
    cmp qword ptr [0x403088], 0x1000
    jne failure

    mov edi, 2
    mov esi, 7
    xor edx, edx
    mov r10d, 0x403090
    mov eax, 302
    syscall
    cmp rax, -3
    jne failure

    xor edi, edi
    mov esi, 99
    xor edx, edx
    mov r10d, 0x403090
    mov eax, 302
    syscall
    cmp rax, -22
    jne failure

    xor edi, edi
    mov esi, 7
    mov edx, 0x403080
    xor r10d, r10d
    mov eax, 302
    syscall
    cmp rax, -1
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4041f8], rax
    xor edi, edi
    mov esi, 7
    xor edx, edx
    mov r10d, 0x4041f8
    mov eax, 302
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4041f8], rax
    jne failure

    xor edi, edi
    mov esi, 7
    xor edx, edx
    mov r10d, 0x403090
    mov eax, 302
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x403090], 3
    jne failure
    cmp qword ptr [0x403098], 3
    jne failure

    mov edi, 2
    mov esi, 8
    mov edx, 0x4030a0
    mov eax, 204
    syscall
    cmp rax, -3
    jne failure

    xor edi, edi
    mov esi, 7
    mov edx, 0x4030a0
    mov eax, 204
    syscall
    cmp rax, -22
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4041fc], rax
    xor edi, edi
    mov esi, 8
    mov edx, 0x4041fc
    mov eax, 204
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4041fc], rax
    jne failure

    xor edi, edi
    mov esi, 8
    mov edx, 0x4030a0
    mov eax, 204
    syscall
    cmp rax, 8
    jne failure
    cmp qword ptr [0x4030a0], 1
    jne failure

    mov eax, 203
    syscall
    test rax, rax
    jne failure
    mov qword ptr [0x4030a0], 0
    mov eax, 203
    syscall
    cmp rax, -22
    jne failure
    mov edx, 0x404200
    mov eax, 203
    syscall
    cmp rax, -14
    jne failure
    mov qword ptr [0x4030a0], 1

    mov dword ptr [0x4030a8], 0x12345678
    mov edi, 0x4030a8
    mov esi, 0x404200
    mov edx, 0xdeadbeef
    mov eax, 309
    syscall
    cmp rax, -14
    jne failure
    cmp dword ptr [0x4030a8], 0x12345678
    jne failure

    mov edi, 0x4030a8
    mov esi, 0x4030ac
    mov edx, 0xdeadbeef
    mov eax, 309
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x4030a8], 0
    jne failure
    cmp dword ptr [0x4030ac], 0
    jne failure

    mov edi, -100
    mov esi, 0x404200
    mov edx, 0x80002
    mov eax, 257
    syscall
    cmp rax, -14
    jne failure

    movabs rax, 0x6c756e2f7665642f
    mov qword ptr [0x403340], rax
    mov word ptr [0x403348], 0x006c
    mov edi, -100
    mov esi, 0x403340
    mov edx, 0x80002
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure

    mov edi, 2
    mov esi, 1
    mov eax, 72
    syscall
    cmp rax, 1
    jne failure
    mov esi, 3
    mov eax, 72
    syscall
    cmp rax, 2
    jne failure

    mov esi, 0x404200
    mov edx, 8
    xor eax, eax
    syscall
    test rax, rax
    jne failure
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure

    mov eax, 8
    syscall
    test rax, rax
    jne failure
    mov esi, 0x403350
    mov eax, 5
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x403368], 0x2180
    jne failure

    mov edi, -100
    mov esi, 0x403340
    mov edx, 2
    mov eax, 257
    syscall
    cmp rax, -24
    jne failure

    mov edi, -100
    mov esi, 0x404200
    mov edx, 0x403350
    xor r10d, r10d
    mov eax, 262
    syscall
    cmp rax, -14
    jne failure
    mov esi, 0x403340
    mov edx, 0x404200
    mov eax, 262
    syscall
    cmp rax, -14
    jne failure
    mov edx, 0x403350
    mov r10d, 1
    mov eax, 262
    syscall
    cmp rax, -22
    jne failure
    xor r10d, r10d
    mov eax, 262
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x403368], 0x2180
    jne failure

    mov edi, -100
    mov esi, 0x403340
    mov edx, 6
    mov eax, 269
    syscall
    test rax, rax
    jne failure
    mov edx, 1
    mov eax, 269
    syscall
    cmp rax, -13
    jne failure
    mov esi, 0x404200
    xor edx, edx
    mov eax, 269
    syscall
    cmp rax, -14
    jne failure
    mov esi, 0x403340
    mov edx, 8
    mov eax, 269
    syscall
    cmp rax, -22
    jne failure

    xor edx, edx
    mov r10d, 1
    mov eax, 439
    syscall
    cmp rax, -22
    jne failure
    xor r10d, r10d
    mov eax, 439
    syscall
    test rax, rax
    jne failure

    mov edi, 2
    mov eax, 3
    syscall
    test rax, rax
    jne failure
    mov eax, 3
    syscall
    cmp rax, -9
    jne failure

    movabs rax, 0x72657a2f7665642f
    mov qword ptr [0x403340], rax
    mov word ptr [0x403348], 0x006f
    mov edi, -100
    mov esi, 0x403340
    xor edx, edx
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure
    mov qword ptr [0x403380], -1
    mov edi, 2
    mov esi, 0x403380
    mov edx, 8
    xor eax, eax
    syscall
    cmp rax, 8
    jne failure
    cmp qword ptr [0x403380], 0
    jne failure
    mov eax, 1
    syscall
    cmp rax, -9
    jne failure
    mov edi, -100
    mov esi, 0x403340
    mov edx, 0x403350
    xor r10d, r10d
    mov eax, 262
    syscall
    test rax, rax
    jne failure
    mov edi, 2
    mov eax, 3
    syscall
    test rax, rax
    jne failure

    movabs rax, 0x6e6f632f7665642f
    mov qword ptr [0x403340], rax
    mov dword ptr [0x403348], 0x656c6f73
    mov byte ptr [0x40334c], 0
    mov edi, -100
    mov esi, 0x403340
    mov edx, 1
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure
    movabs rax, 0x2121454349564544
    mov qword ptr [0x403380], rax
    mov byte ptr [0x403388], 10
    mov edi, 2
    mov esi, 0x403380
    mov edx, 9
    mov eax, 1
    syscall
    cmp rax, 9
    jne failure
    mov esi, 0x5413
    mov edx, 0x403390
    mov eax, 16
    syscall
    test rax, rax
    jne failure
    cmp word ptr [0x403390], 24
    jne failure
    cmp word ptr [0x403392], 80
    jne failure
    mov eax, 8
    syscall
    cmp rax, -29
    jne failure
    mov eax, 3
    syscall
    test rax, rax
    jne failure

    movabs rax, 0x7974742f7665642f
    mov qword ptr [0x403340], rax
    mov byte ptr [0x403348], 0
    mov edi, -100
    mov esi, 0x403340
    mov edx, 2
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure
    mov edi, 2
    mov esi, 0x404200
    mov edx, 8
    xor eax, eax
    syscall
    test rax, rax
    jne failure
    mov eax, 3
    syscall
    test rax, rax
    jne failure

    mov edi, -100
    mov esi, 0x403340
    xor edx, edx
    mov r10d, 0x7ff
    mov r8d, 0x404200
    mov eax, 332
    syscall
    cmp rax, -14
    jne failure
    mov r8d, 0x403400
    mov eax, 332
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x403400], 0x7ff
    jne failure
    cmp word ptr [0x40341c], 0x2180
    jne failure
    cmp qword ptr [0x403420], 3
    jne failure
    mov r10d, 0x80000000
    mov eax, 332
    syscall
    cmp rax, -22
    jne failure

    movabs rax, 0x72657a2f7665642f
    mov qword ptr [0x403340], rax
    mov word ptr [0x403348], 0x006f
    mov edi, -100
    mov esi, 0x403340
    xor edx, edx
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure
    mov qword ptr [0x403540], -1
    mov qword ptr [0x403550], -1
    mov qword ptr [0x403500], 0x403540
    mov qword ptr [0x403508], 4
    mov qword ptr [0x403510], 0x404200
    mov qword ptr [0x403518], 4
    mov edi, 2
    mov esi, 0x403500
    mov edx, 2
    mov eax, 19
    syscall
    cmp rax, -14
    jne failure
    cmp qword ptr [0x403540], -1
    jne failure
    mov qword ptr [0x403510], 0x403550
    mov eax, 19
    syscall
    cmp rax, 8
    jne failure
    cmp dword ptr [0x403540], 0
    jne failure
    cmp dword ptr [0x403550], 0
    jne failure
    mov edi, 2
    mov eax, 3
    syscall
    test rax, rax
    jne failure

    movabs rax, 0x6c756e2f7665642f
    mov qword ptr [0x403340], rax
    mov word ptr [0x403348], 0x006c
    mov edi, -100
    mov esi, 0x403340
    mov edx, 1
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure
    mov qword ptr [0x403500], 0x404200
    mov qword ptr [0x403508], 8
    mov edi, 2
    mov esi, 0x403500
    mov edx, 1
    mov eax, 20
    syscall
    cmp rax, 8
    jne failure
    mov edi, 2
    mov eax, 3
    syscall
    test rax, rax
    jne failure

    mov dword ptr [0x403340], 0x7665642f
    mov byte ptr [0x403344], 0
    mov edi, -100
    mov esi, 0x403340
    mov edx, 0x10000
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure
    mov edi, 2
    mov esi, 0x403700
    mov eax, 5
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x403718], 0x416d
    jne failure
    mov esi, 0x403800
    mov edx, 151
    mov eax, 217
    syscall
    cmp rax, -22
    jne failure
    mov esi, 0x404200
    mov edx, 152
    mov eax, 217
    syscall
    cmp rax, -14
    jne failure
    mov esi, 0x403800
    mov eax, 217
    syscall
    cmp rax, 152
    jne failure
    cmp byte ptr [0x403813], '.'
    jne failure
    cmp byte ptr [0x403843], 'n'
    jne failure
    mov eax, 217
    syscall
    test rax, rax
    jne failure
    xor esi, esi
    xor edx, edx
    mov eax, 8
    syscall
    test rax, rax
    jne failure
    mov esi, 0x403800
    mov edx, 152
    mov eax, 217
    syscall
    cmp rax, 152
    jne failure
    mov eax, 3
    syscall
    test rax, rax
    jne failure

    mov edi, 0x404200
    mov eax, 80
    syscall
    cmp rax, -14
    jne failure

    mov word ptr [0x4030b0], 0x2f
    mov edi, 0x4030b0
    mov eax, 80
    syscall
    test rax, rax
    jne failure

    movabs rax, 0x676e697373696d2f
    mov qword ptr [0x4030b0], rax
    mov byte ptr [0x4030b8], 0
    mov edi, 0x4030b0
    mov eax, 80
    syscall
    cmp rax, -2
    jne failure

    mov edi, 16
    mov esi, 0x4030d0
    mov eax, 157
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x4030d0], 0x74696e69
    jne failure
    cmp dword ptr [0x4030d4], 0
    jne failure

    mov edi, 15
    mov esi, 0x404200
    mov eax, 157
    syscall
    cmp rax, -14
    jne failure

    movabs rax, 0x72702d78756e6967
    mov qword ptr [0x4030c0], rax
    mov dword ptr [0x4030c8], 0x0065626f
    mov edi, 15
    mov esi, 0x4030c0
    mov eax, 157
    syscall
    test rax, rax
    jne failure

    mov edi, 16
    mov esi, 0x404200
    mov eax, 157
    syscall
    cmp rax, -14
    jne failure

    mov edi, 16
    mov esi, 0x4030d0
    mov eax, 157
    syscall
    test rax, rax
    jne failure
    movabs rax, 0x72702d78756e6967
    cmp qword ptr [0x4030d0], rax
    jne failure
    cmp dword ptr [0x4030d8], 0x0065626f
    jne failure
    cmp dword ptr [0x4030dc], 0
    jne failure

    mov edi, 99
    mov eax, 157
    syscall
    cmp rax, -22
    jne failure

    mov dword ptr [0x4030c0], 0x74696e69
    mov byte ptr [0x4030c4], 0
    mov edi, 15
    mov esi, 0x4030c0
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
    mov esi, 0x404200
    mov rdx, -1
    xor eax, eax
    syscall
    cmp rax, -9
    jne failure

    xor edi, edi
    mov esi, 0x404200
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
    mov esi, 0x403200
    mov eax, 5
    syscall
    cmp rax, -9
    jne failure

    xor edi, edi
    mov esi, 0x5413
    mov edx, 0x403280
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
    mov esi, 0x404200
    mov eax, 115
    syscall
    cmp rax, 1
    jne failure

    mov dword ptr [0x403060], 0x12345678
    mov edi, 1
    mov esi, 0x404200
    mov eax, 115
    syscall
    cmp rax, -14
    jne failure
    cmp dword ptr [0x403060], 0x12345678
    jne failure

    mov edi, 1
    mov esi, 0x403060
    mov eax, 115
    syscall
    cmp rax, 1
    jne failure
    cmp dword ptr [0x403060], 0
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

    mov dword ptr [0x403060], 0x20080522
    mov dword ptr [0x403064], 0
    mov dword ptr [0x403068], 0x12345678
    mov edi, 0x403060
    mov esi, 0x404200
    mov eax, 125
    syscall
    cmp rax, -14
    jne failure
    cmp dword ptr [0x403068], 0x12345678
    jne failure

    mov edi, 0x403060
    mov esi, 0x403068
    mov eax, 125
    syscall
    test rax, rax
    jne failure
    mov eax, dword ptr [0x403068]
    or eax, dword ptr [0x40306c]
    or eax, dword ptr [0x403070]
    or eax, dword ptr [0x403074]
    or eax, dword ptr [0x403078]
    or eax, dword ptr [0x40307c]
    jne failure

    mov dword ptr [0x403060], 0
    mov edi, 0x403060
    xor esi, esi
    mov eax, 125
    syscall
    cmp rax, -22
    jne failure
    cmp dword ptr [0x403060], 0x20080522
    jne failure

    mov dword ptr [0x403064], 2
    mov eax, 125
    syscall
    cmp rax, -3
    jne failure

    mov dword ptr [0x403064], 0
    mov edi, 0x403060
    mov esi, 0x403068
    mov eax, 126
    syscall
    test rax, rax
    jne failure
    mov dword ptr [0x403068], 1
    mov eax, 126
    syscall
    cmp rax, -1
    jne failure
    mov dword ptr [0x403068], 0

    xor edi, edi
    xor esi, esi
    mov eax, 116
    syscall
    cmp rax, -1
    jne failure

    mov dword ptr [0x403060], 0x12345678
    mov dword ptr [0x403064], 0x12345678
    mov edi, 0x403060
    mov esi, 0x403064
    mov edx, 0x404200
    mov eax, 118
    syscall
    cmp rax, -14
    jne failure
    cmp dword ptr [0x403060], 0x12345678
    jne failure
    cmp dword ptr [0x403064], 0x12345678
    jne failure

    mov edi, 0x403060
    mov esi, 0x403064
    mov edx, 0x403068
    mov eax, 118
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x403060], 0
    jne failure
    cmp dword ptr [0x403064], 0
    jne failure
    cmp dword ptr [0x403068], 0
    jne failure

    mov dword ptr [0x403060], 0x12345678
    mov dword ptr [0x403064], 0x12345678
    mov dword ptr [0x403068], 0x12345678
    mov edi, 0x403060
    mov esi, 0x403064
    mov edx, 0x403068
    mov eax, 120
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x403060], 0
    jne failure
    cmp dword ptr [0x403064], 0
    jne failure
    cmp dword ptr [0x403068], 0
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

    mov edi, 0x4031b0
    mov esi, 23
    mov eax, 273
    syscall
    cmp rax, -22
    jne failure

    mov edi, 0x4041f0
    mov esi, 24
    mov eax, 273
    syscall
    cmp rax, -14
    jne failure

    mov edi, 0x4031b0
    mov esi, 24
    mov eax, 273
    syscall
    test rax, rax
    jne failure

    mov edi, 2
    mov esi, 0x4031c8
    mov edx, 0x4031d0
    mov eax, 274
    syscall
    cmp rax, -3
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4031c8], rax
    xor edi, edi
    mov esi, 0x4031c8
    mov edx, 0x4041fc
    mov eax, 274
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4031c8], rax
    jne failure

    xor edi, edi
    mov esi, 0x4031c8
    mov edx, 0x4031d0
    mov eax, 274
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4031c8], 0x4031b0
    jne failure
    cmp qword ptr [0x4031d0], 24
    jne failure

    mov edi, 0x404001
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    syscall
    cmp rax, -22
    jne failure

    mov edi, 0x404000
    mov esi, 4096
    mov edx, 4
    mov eax, 10
    syscall
    cmp rax, -22
    jne failure

    movabs rax, 0x0a212145424f5250
    mov qword ptr [0x404000], rax
    mov edi, 0x404000
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    syscall
    test rax, rax
    jne failure

    mov edi, 0x404000
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    syscall
    cmp rax, -22
    jne failure

    mov edi, 0x404000
    mov eax, 63
    syscall
    cmp rax, -14
    jne failure

    mov edi, 1
    mov esi, 0x404000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x403280], rax
    mov qword ptr [0x403298], rax
    mov edi, 1
    mov esi, 0x5401
    mov edx, 0x403280
    mov eax, 16
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x403280], 0
    jne failure
    cmp qword ptr [0x403298], 0
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

    movabs rax, 0x1122334455667788
    mov qword ptr [0x403ff8], rax
    mov edi, 1
    mov esi, 0x403ff8
    mov eax, 228
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x403ff8], rax
    jne failure

    xor edi, edi
    mov esi, 0x403900
    mov eax, 228
    syscall
    cmp rax, -22
    jne failure

    mov edi, 1
    mov esi, 0x403900
    mov eax, 228
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x403900], 0
    jl failure
    cmp qword ptr [0x403908], 1000000000
    jae failure

    mov edi, 1
    mov esi, 0x403910
    mov eax, 228
    syscall
    test rax, rax
    jne failure
    mov rax, qword ptr [0x403910]
    cmp rax, qword ptr [0x403900]
    jb failure
    ja clock_advanced
    mov rax, qword ptr [0x403918]
    cmp rax, qword ptr [0x403908]
    jb failure
clock_advanced:
    cmp qword ptr [0x403918], 1000000000
    jae failure

    mov edi, 4
    mov esi, 0x403920
    mov eax, 228
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x403928], 1000000000
    jae failure

    mov edi, 0x404200
    mov eax, 218
    syscall
    cmp rax, -14
    jne failure

    mov dword ptr [0x403190], 0x12345678
    mov edi, 0x403190
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

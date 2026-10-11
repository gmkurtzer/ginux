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
    mov esi, 0x408ff9
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, -14
    jne failure

    xor edi, edi
    xor esi, esi
    xor edx, edx
    mov eax, 7
    syscall
    test rax, rax
    jne failure
    mov edx, 1
    mov eax, 7
    syscall
    cmp rax, -38
    jne failure

    mov edi, 0x404c80
    mov esi, 5
    xor edx, edx
    mov eax, 7
    syscall
    cmp rax, -22
    jne failure
    mov edi, 0x4051fc
    mov esi, 1
    mov eax, 7
    syscall
    cmp rax, -14
    jne failure

    mov dword ptr [0x404c80], 0
    mov word ptr [0x404c84], 1
    mov word ptr [0x404c86], -1
    mov dword ptr [0x404c88], 1
    mov word ptr [0x404c8c], 4
    mov word ptr [0x404c8e], -1
    mov dword ptr [0x404c90], 99
    mov word ptr [0x404c94], 0
    mov word ptr [0x404c96], -1
    mov dword ptr [0x404c98], -1
    mov word ptr [0x404c9c], -1
    mov word ptr [0x404c9e], -1
    mov edi, 0x404c80
    mov esi, 4
    mov edx, -1
    mov eax, 7
    syscall
    cmp rax, 3
    jne failure
    cmp word ptr [0x404c86], 0x10
    jne failure
    cmp word ptr [0x404c8e], 4
    jne failure
    cmp word ptr [0x404c96], 0x20
    jne failure
    cmp word ptr [0x404c9e], 0
    jne failure

    mov dword ptr [0x404ca0], 1
    mov word ptr [0x404ca4], 0
    mov word ptr [0x404ca6], 0x7777
    mov edi, 0x404ca0
    mov esi, 1
    xor edx, edx
    mov eax, 7
    syscall
    test rax, rax
    jne failure
    cmp word ptr [0x404ca6], 0
    jne failure
    mov word ptr [0x404ca6], 0x7777
    mov edx, 1
    mov eax, 7
    syscall
    cmp rax, -38
    jne failure
    cmp word ptr [0x404ca6], 0x7777
    jne failure

    xor edi, edi
    mov eax, 12
    syscall
    cmp rax, 0x406000
    jne failure
    mov edi, 0x406080
    mov eax, 12
    syscall
    cmp rax, 0x406080
    jne failure
    movabs rax, 0x0a212145424f5250
    mov qword ptr [0x406000], rax
    mov edi, 1
    mov esi, 0x406000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure
    mov edi, 1
    mov esi, 0x406079
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x4845415042524b21
    mov qword ptr [0x406000], rax
    mov edi, 0x407001
    mov eax, 12
    syscall
    cmp rax, 0x406080
    jne failure
    mov edi, 0x406040
    mov eax, 12
    syscall
    cmp rax, 0x406040
    jne failure

    xor edi, edi
    mov esi, 4096
    mov edx, 7
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, -22
    jne failure

    xor edi, edi
    mov esi, 4096
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40a000
    jne failure
    cmp qword ptr [0x40a000], 0
    jne failure

    mov edi, 0x40a000
    mov eax, 63
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x40a000], 0x756e6947
    jne failure

    xor edi, edi
    mov esi, 4096
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, -12
    jne failure

    mov edi, 0x40a000
    mov esi, 4095
    mov eax, 11
    syscall
    cmp rax, -22
    jne failure
    mov esi, 4096
    mov eax, 11
    syscall
    test rax, rax
    jne failure

    mov edi, 1
    mov esi, 0x40a000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, -14
    jne failure

    xor edi, edi
    mov esi, 4096
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40a000
    jne failure
    cmp qword ptr [0x40a000], 0
    jne failure
    mov qword ptr [0x40a000], 2

    mov edi, 0x40a000
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    syscall
    test rax, rax
    jne failure
    mov eax, 10
    syscall
    cmp rax, -22
    jne failure

    mov edi, 0x40a000
    mov eax, 63
    syscall
    cmp rax, -14
    jne failure

    mov edi, 2
    mov esi, 0x40a000
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    mov qword ptr [0x404940], 0
    mov esi, 0x404940
    mov eax, 14
    syscall
    test rax, rax
    jne failure

    mov edi, 0x40a000
    mov esi, 4096
    mov edx, 3
    mov eax, 28
    syscall
    cmp rax, -22
    jne failure
    cmp qword ptr [0x40a000], 2
    jne failure
    mov edx, 4
    mov eax, 28
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x40a000], 0
    jne failure

    mov edi, 0x40a000
    mov esi, 4096
    mov eax, 11
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov esi, 4096
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40a000
    jne failure
    cmp qword ptr [0x40a000], 0
    jne failure

    mov edi, 0x40a001
    mov esi, 4096
    mov edx, 0x404948
    mov eax, 27
    syscall
    cmp rax, -22
    jne failure
    mov edi, 0x40a000
    mov edx, 0x405200
    mov eax, 27
    syscall
    cmp rax, -14
    jne failure
    mov byte ptr [0x404948], 0
    mov edx, 0x404948
    mov eax, 27
    syscall
    test rax, rax
    jne failure
    cmp byte ptr [0x404948], 1
    jne failure

    mov edi, 0x40a000
    mov eax, 63
    syscall
    test rax, rax
    jne failure
    mov edi, 0x40a000
    mov esi, 4096
    mov eax, 11
    syscall
    test rax, rax
    jne failure
    mov edx, 0x404948
    mov eax, 27
    syscall
    cmp rax, -12
    jne failure
    mov eax, 11
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov esi, 8192
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40a000
    jne failure
    cmp qword ptr [0x40a000], 0
    jne failure
    cmp qword ptr [0x40b000], 0
    jne failure

    mov qword ptr [0x40b000], -1
    mov edi, 0x40af80
    mov eax, 63
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x40af80], 0x756e6947
    jne failure
    cmp word ptr [0x40b000], 0
    jne failure

    mov byte ptr [0x40494a], 0
    mov byte ptr [0x40494b], 0
    mov edi, 0x40a000
    mov esi, 8192
    mov edx, 0x40494a
    mov eax, 27
    syscall
    test rax, rax
    jne failure
    cmp byte ptr [0x40494a], 1
    jne failure
    cmp byte ptr [0x40494b], 1
    jne failure

    mov qword ptr [0x40a000], 1
    mov qword ptr [0x40b000], 2
    mov edx, 4
    mov eax, 28
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x40a000], 0
    jne failure
    cmp qword ptr [0x40b000], 0
    jne failure

    mov edi, 0x40a000
    mov esi, 8192
    mov edx, 1
    mov eax, 10
    syscall
    test rax, rax
    jne failure
    mov eax, 10
    syscall
    cmp rax, -22
    jne failure
    mov edi, 0x40af80
    mov eax, 63
    syscall
    cmp rax, -14
    jne failure
    mov edi, 0x40a000

    mov esi, 4096
    mov eax, 11
    syscall
    cmp rax, -22
    jne failure
    mov esi, 8192
    mov eax, 11
    syscall
    test rax, rax
    jne failure
    mov edi, 1
    mov esi, 0x40b000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, -14
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
    mov esi, 0x405200
    mov eax, 143
    syscall
    cmp rax, -14
    jne failure
    mov dword ptr [0x404060], 0x12345678
    mov esi, 0x404060
    mov eax, 143
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x404060], 0
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
    mov esi, 0x405200
    mov eax, 142
    syscall
    cmp rax, -14
    jne failure
    mov dword ptr [0x404060], 0
    mov esi, 0x404060
    mov eax, 142
    syscall
    test rax, rax
    jne failure
    xor esi, esi
    mov edx, 0x404060
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
    mov esi, 0x404000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, -9
    jne failure

    mov edi, 1
    mov esi, 0x404000
    mov edx, 257
    mov eax, 1
    syscall
    cmp rax, -22
    jne failure

    mov qword ptr [0x4040c0], 0x40102
    mov edi, 99
    xor esi, esi
    mov edx, 0x4040e0
    mov r10d, 7
    mov eax, 14
    syscall
    cmp rax, -22
    jne failure

    mov edi, 99
    xor esi, esi
    mov edx, 0x4040e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4040e0], 0
    jne failure

    xor edi, edi
    mov esi, 0x4040c0
    mov edx, 0x4040e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4040e0], 0
    jne failure

    mov edi, 99
    xor esi, esi
    mov edx, 0x4040e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4040e0], 2
    jne failure

    mov qword ptr [0x4040c0], 8
    xor edi, edi
    mov esi, 0x4040c0
    mov edx, 0x405200
    mov r10d, 8
    mov eax, 14
    syscall
    cmp rax, -14
    jne failure

    mov edi, 99
    xor esi, esi
    mov edx, 0x4040e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4040e0], 2
    jne failure

    mov qword ptr [0x4040c0], 2
    mov edi, 1
    mov esi, 0x4040c0
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure

    mov edi, 99
    xor esi, esi
    mov edx, 0x4040e0
    mov r10d, 8
    mov eax, 14
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4040e0], 0
    jne failure

    xor edi, edi
    mov esi, 0x405200
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

    mov qword ptr [0x404100], 1
    mov qword ptr [0x404108], 0x1234
    mov qword ptr [0x404110], 0x400100
    mov qword ptr [0x404118], 0x40102
    mov edi, 9
    mov esi, 0x404100
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    cmp rax, -22
    jne failure

    mov edi, 10
    mov esi, 0x405200
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    cmp rax, -14
    jne failure

    mov edi, 10
    mov esi, 0x404100
    mov edx, 0x404120
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404120], 0
    jne failure
    cmp qword ptr [0x404138], 0
    jne failure

    mov edi, 10
    xor esi, esi
    mov edx, 0x404120
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404120], 1
    jne failure
    cmp qword ptr [0x404128], 0x1234
    jne failure
    cmp qword ptr [0x404130], 0x400100
    jne failure
    cmp qword ptr [0x404138], 2
    jne failure

    mov qword ptr [0x404100], 0
    mov qword ptr [0x404108], 0
    mov qword ptr [0x404110], 0
    mov qword ptr [0x404118], 0
    mov edi, 10
    mov esi, 0x404100
    mov edx, 0x405200
    mov r10d, 8
    mov eax, 13
    syscall
    cmp rax, -14
    jne failure

    mov edi, 10
    xor esi, esi
    mov edx, 0x404120
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404120], 1
    jne failure

    mov edi, 10
    mov esi, 0x404100
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    test rax, rax
    jne failure

    xor edi, edi
    mov esi, 0x404158
    mov eax, 131
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404158], 0
    jne failure
    cmp dword ptr [0x404160], 2
    jne failure
    cmp qword ptr [0x404168], 0
    jne failure

    xor edi, edi
    mov esi, 0x405200
    mov eax, 131
    syscall
    cmp rax, -14
    jne failure

    mov qword ptr [0x404140], 0x404800
    mov qword ptr [0x404148], 0
    mov qword ptr [0x404150], 2048
    mov edi, 0x404140
    mov esi, 0x404158
    mov eax, 131
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x404160], 2
    jne failure

    xor edi, edi
    mov esi, 0x404158
    mov eax, 131
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404158], 0x404800
    jne failure
    cmp dword ptr [0x404160], 0
    jne failure
    cmp qword ptr [0x404168], 2048
    jne failure

    mov qword ptr [0x404140], 0x405000
    mov qword ptr [0x404148], 0
    mov qword ptr [0x404150], 2048
    mov edi, 0x404140
    xor esi, esi
    mov eax, 131
    syscall
    cmp rax, -14
    jne failure

    mov qword ptr [0x404140], 0x404800
    mov qword ptr [0x404148], 0
    mov qword ptr [0x404150], 1024
    mov edi, 0x404140
    xor esi, esi
    mov eax, 131
    syscall
    cmp rax, -12
    jne failure

    mov qword ptr [0x404140], 0
    mov qword ptr [0x404148], 2
    mov qword ptr [0x404150], 0
    mov edi, 0x404140
    xor esi, esi
    mov eax, 131
    syscall
    test rax, rax
    jne failure

    mov edi, 1
    mov esi, 0x404000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure

    mov edi, 0x1002
    mov esi, 0x404000
    mov eax, 158
    syscall
    test rax, rax
    jne failure
    mov rbx, qword ptr fs:[0]
    movabs rax, 0x0a212145424f5250
    cmp rbx, rax
    jne failure

    mov edi, 0x1003
    mov esi, 0x405200
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
    mov esi, 0x404008
    mov eax, 158
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404008], 0x404000
    jne failure

    mov edi, 0x404020
    mov esi, 1
    mov eax, 79
    syscall
    cmp rax, -34
    jne failure
    mov edi, 0x405200
    mov esi, 2
    mov eax, 79
    syscall
    cmp rax, -14
    jne failure
    mov edi, 0x404020
    mov esi, 2
    mov eax, 79
    syscall
    cmp rax, 2
    jne failure
    cmp word ptr [0x404020], 0x002f
    jne failure

    lea rdi, [rip + proc_self_exe]
    mov esi, 0x404040
    xor edx, edx
    mov eax, 89
    syscall
    cmp rax, -22
    jne failure
    mov edi, 0x409000
    mov esi, 0x404040
    mov edx, 5
    mov eax, 89
    syscall
    cmp rax, -14
    jne failure
    lea rdi, [rip + proc_self_exe]
    mov esi, 0x405200
    mov edx, 5
    mov eax, 89
    syscall
    cmp rax, -14
    jne failure
    lea rdi, [rip + proc_self_exe]
    mov esi, 0x404040
    mov edx, 5
    mov eax, 89
    syscall
    cmp rax, 5
    jne failure
    cmp dword ptr [0x404040], 0x696e692f
    jne failure
    cmp byte ptr [0x404044], 't'
    jne failure

    mov edi, -100
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x404040
    mov r10d, 5
    mov eax, 267
    syscall
    cmp rax, 5
    jne failure
    cmp dword ptr [0x404040], 0x696e692f
    jne failure
    cmp byte ptr [0x404044], 't'
    jne failure

    mov edi, -99
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x404040
    mov r10d, 5
    mov eax, 267
    syscall
    cmp rax, -9
    jne failure

    mov edi, -100
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x404040
    xor r10d, r10d
    mov eax, 267
    syscall
    cmp rax, -22
    jne failure

    mov edi, -100
    lea rsi, [rip + proc_self_exe]
    mov edx, 0x405200
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

    mov edi, 0x405200
    mov eax, 63
    syscall
    cmp rax, -14
    jne failure

    mov edi, 0x404000
    mov eax, 63
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [rdi], 0x756e6947
    jne failure

    mov edi, 2
    mov esi, 0x404200
    mov eax, 5
    syscall
    cmp rax, -9
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4051c0], rax
    mov edi, 1
    mov esi, 0x4051c0
    mov eax, 5
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4051c0], rax
    jne failure

    mov edi, 1
    mov esi, 0x404200
    mov eax, 5
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404210], 1
    jne failure
    cmp dword ptr [0x404218], 0x2180
    jne failure
    cmp qword ptr [0x404238], 4096
    jne failure

    mov edi, 2
    mov esi, 0x5413
    mov edx, 0x404280
    mov eax, 16
    syscall
    cmp rax, -9
    jne failure

    mov edi, 1
    mov esi, 0x5402
    mov edx, 0x404280
    mov eax, 16
    syscall
    cmp rax, -25
    jne failure

    mov edi, 1
    mov esi, 0x5413
    mov edx, 0x4051fc
    mov eax, 16
    syscall
    cmp rax, -14
    jne failure

    mov edi, 1
    mov esi, 0x5413
    mov edx, 0x404280
    mov eax, 16
    syscall
    test rax, rax
    jne failure
    cmp word ptr [0x404280], 24
    jne failure
    cmp word ptr [0x404282], 80
    jne failure
    cmp dword ptr [0x404284], 0
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
    mov qword ptr [0x404300], rax
    mov qword ptr [0x404308], 4
    mov qword ptr [0x404310], 0x405200
    mov qword ptr [0x404318], 4
    mov edi, 1
    mov esi, 0x404300
    mov edx, 2
    mov eax, 20
    syscall
    cmp rax, -14
    jne failure

    lea rax, [rip + readable_payload + 4]
    mov qword ptr [0x404310], rax
    mov edi, 2
    mov esi, 0x404300
    mov edx, 2
    mov eax, 20
    syscall
    cmp rax, -9
    jne failure

    mov edi, 1
    mov esi, 0x404300
    mov edx, 5
    mov eax, 20
    syscall
    cmp rax, -22
    jne failure

    mov edi, 1
    mov esi, 0x404300
    mov edx, 2
    mov eax, 20
    syscall
    cmp rax, 8
    jne failure

    mov edi, 99
    mov esi, 0x404080
    mov eax, 97
    syscall
    cmp rax, -22
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4051f8], rax
    mov edi, 3
    mov esi, 0x4051f8
    mov eax, 97
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4051f8], rax
    jne failure

    mov edi, 3
    mov esi, 0x404080
    mov eax, 97
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404080], 0x1000
    jne failure
    cmp qword ptr [0x404088], 0x1000
    jne failure

    mov edi, 2
    mov esi, 7
    xor edx, edx
    mov r10d, 0x404090
    mov eax, 302
    syscall
    cmp rax, -3
    jne failure

    xor edi, edi
    mov esi, 99
    xor edx, edx
    mov r10d, 0x404090
    mov eax, 302
    syscall
    cmp rax, -22
    jne failure

    xor edi, edi
    mov esi, 7
    mov edx, 0x404080
    xor r10d, r10d
    mov eax, 302
    syscall
    cmp rax, -1
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4051f8], rax
    xor edi, edi
    mov esi, 7
    xor edx, edx
    mov r10d, 0x4051f8
    mov eax, 302
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4051f8], rax
    jne failure

    xor edi, edi
    mov esi, 7
    xor edx, edx
    mov r10d, 0x404090
    mov eax, 302
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404090], 3
    jne failure
    cmp qword ptr [0x404098], 3
    jne failure

    mov edi, 2
    mov esi, 8
    mov edx, 0x4040a0
    mov eax, 204
    syscall
    cmp rax, -3
    jne failure

    xor edi, edi
    mov esi, 7
    mov edx, 0x4040a0
    mov eax, 204
    syscall
    cmp rax, -22
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4051fc], rax
    xor edi, edi
    mov esi, 8
    mov edx, 0x4051fc
    mov eax, 204
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4051fc], rax
    jne failure

    xor edi, edi
    mov esi, 8
    mov edx, 0x4040a0
    mov eax, 204
    syscall
    cmp rax, 8
    jne failure
    cmp qword ptr [0x4040a0], 1
    jne failure

    mov eax, 203
    syscall
    test rax, rax
    jne failure
    mov qword ptr [0x4040a0], 0
    mov eax, 203
    syscall
    cmp rax, -22
    jne failure
    mov edx, 0x405200
    mov eax, 203
    syscall
    cmp rax, -14
    jne failure
    mov qword ptr [0x4040a0], 1

    mov dword ptr [0x4040a8], 0x12345678
    mov edi, 0x4040a8
    mov esi, 0x405200
    mov edx, 0xdeadbeef
    mov eax, 309
    syscall
    cmp rax, -14
    jne failure
    cmp dword ptr [0x4040a8], 0x12345678
    jne failure

    mov edi, 0x4040a8
    mov esi, 0x4040ac
    mov edx, 0xdeadbeef
    mov eax, 309
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x4040a8], 0
    jne failure
    cmp dword ptr [0x4040ac], 0
    jne failure

    mov edi, -100
    mov esi, 0x405200
    mov edx, 0x80002
    mov eax, 257
    syscall
    cmp rax, -14
    jne failure

    movabs rax, 0x6c756e2f7665642f
    mov qword ptr [0x404340], rax
    mov word ptr [0x404348], 0x006c
    mov edi, -100
    mov esi, 0x404340
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

    mov esi, 0x405200
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
    mov esi, 0x404350
    mov eax, 5
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x404368], 0x2180
    jne failure

    mov edi, -100
    mov esi, 0x404340
    mov edx, 2
    mov eax, 257
    syscall
    cmp rax, -24
    jne failure

    mov edi, -100
    mov esi, 0x405200
    mov edx, 0x404350
    xor r10d, r10d
    mov eax, 262
    syscall
    cmp rax, -14
    jne failure
    mov esi, 0x404340
    mov edx, 0x405200
    mov eax, 262
    syscall
    cmp rax, -14
    jne failure
    mov edx, 0x404350
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
    cmp dword ptr [0x404368], 0x2180
    jne failure

    mov edi, -100
    mov esi, 0x404340
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
    mov esi, 0x405200
    xor edx, edx
    mov eax, 269
    syscall
    cmp rax, -14
    jne failure
    mov esi, 0x404340
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
    mov qword ptr [0x404340], rax
    mov word ptr [0x404348], 0x006f
    mov edi, -100
    mov esi, 0x404340
    xor edx, edx
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure
    mov qword ptr [0x404380], -1
    mov edi, 2
    mov esi, 0x404380
    mov edx, 8
    xor eax, eax
    syscall
    cmp rax, 8
    jne failure
    cmp qword ptr [0x404380], 0
    jne failure
    mov eax, 1
    syscall
    cmp rax, -9
    jne failure
    mov edi, -100
    mov esi, 0x404340
    mov edx, 0x404350
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
    mov qword ptr [0x404340], rax
    mov dword ptr [0x404348], 0x656c6f73
    mov byte ptr [0x40434c], 0
    mov edi, -100
    mov esi, 0x404340
    mov edx, 1
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure
    movabs rax, 0x2121454349564544
    mov qword ptr [0x404380], rax
    mov byte ptr [0x404388], 10
    mov edi, 2
    mov esi, 0x404380
    mov edx, 9
    mov eax, 1
    syscall
    cmp rax, 9
    jne failure
    mov esi, 0x5413
    mov edx, 0x404390
    mov eax, 16
    syscall
    test rax, rax
    jne failure
    cmp word ptr [0x404390], 24
    jne failure
    cmp word ptr [0x404392], 80
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
    mov qword ptr [0x404340], rax
    mov byte ptr [0x404348], 0
    mov edi, -100
    mov esi, 0x404340
    mov edx, 2
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure
    mov edi, 2
    mov esi, 0x405200
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
    mov esi, 0x404340
    xor edx, edx
    mov r10d, 0x7ff
    mov r8d, 0x405200
    mov eax, 332
    syscall
    cmp rax, -14
    jne failure
    mov r8d, 0x404400
    mov eax, 332
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x404400], 0x7ff
    jne failure
    cmp word ptr [0x40441c], 0x2180
    jne failure
    cmp qword ptr [0x404420], 3
    jne failure
    mov r10d, 0x80000000
    mov eax, 332
    syscall
    cmp rax, -22
    jne failure

    movabs rax, 0x72657a2f7665642f
    mov qword ptr [0x404340], rax
    mov word ptr [0x404348], 0x006f
    mov edi, -100
    mov esi, 0x404340
    xor edx, edx
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure
    mov qword ptr [0x404540], -1
    mov qword ptr [0x404550], -1
    mov qword ptr [0x404500], 0x404540
    mov qword ptr [0x404508], 4
    mov qword ptr [0x404510], 0x405200
    mov qword ptr [0x404518], 4
    mov edi, 2
    mov esi, 0x404500
    mov edx, 2
    mov eax, 19
    syscall
    cmp rax, -14
    jne failure
    cmp qword ptr [0x404540], -1
    jne failure
    mov qword ptr [0x404510], 0x404550
    mov eax, 19
    syscall
    cmp rax, 8
    jne failure
    cmp dword ptr [0x404540], 0
    jne failure
    cmp dword ptr [0x404550], 0
    jne failure
    mov edi, 2
    mov eax, 3
    syscall
    test rax, rax
    jne failure

    movabs rax, 0x6c756e2f7665642f
    mov qword ptr [0x404340], rax
    mov word ptr [0x404348], 0x006c
    mov edi, -100
    mov esi, 0x404340
    mov edx, 1
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure
    mov qword ptr [0x404500], 0x405200
    mov qword ptr [0x404508], 8
    mov edi, 2
    mov esi, 0x404500
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

    mov dword ptr [0x404340], 0x7665642f
    mov byte ptr [0x404344], 0
    mov edi, -100
    mov esi, 0x404340
    mov edx, 0x10000
    mov eax, 257
    syscall
    cmp rax, 2
    jne failure
    mov edi, 2
    mov esi, 0x404700
    mov eax, 5
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x404718], 0x416d
    jne failure
    mov esi, 0x404800
    mov edx, 151
    mov eax, 217
    syscall
    cmp rax, -22
    jne failure
    mov esi, 0x405200
    mov edx, 152
    mov eax, 217
    syscall
    cmp rax, -14
    jne failure
    mov esi, 0x404800
    mov eax, 217
    syscall
    cmp rax, 152
    jne failure
    cmp byte ptr [0x404813], '.'
    jne failure
    cmp byte ptr [0x404843], 'n'
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
    mov esi, 0x404800
    mov edx, 152
    mov eax, 217
    syscall
    cmp rax, 152
    jne failure
    mov eax, 3
    syscall
    test rax, rax
    jne failure

    mov edi, 0x405200
    mov eax, 80
    syscall
    cmp rax, -14
    jne failure

    mov word ptr [0x4040b0], 0x2f
    mov edi, 0x4040b0
    mov eax, 80
    syscall
    test rax, rax
    jne failure

    movabs rax, 0x676e697373696d2f
    mov qword ptr [0x4040b0], rax
    mov byte ptr [0x4040b8], 0
    mov edi, 0x4040b0
    mov eax, 80
    syscall
    cmp rax, -2
    jne failure

    mov edi, 16
    mov esi, 0x4040d0
    mov eax, 157
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x4040d0], 0x74696e69
    jne failure
    cmp dword ptr [0x4040d4], 0
    jne failure

    mov edi, 15
    mov esi, 0x405200
    mov eax, 157
    syscall
    cmp rax, -14
    jne failure

    movabs rax, 0x72702d78756e6967
    mov qword ptr [0x4040c0], rax
    mov dword ptr [0x4040c8], 0x0065626f
    mov edi, 15
    mov esi, 0x4040c0
    mov eax, 157
    syscall
    test rax, rax
    jne failure

    mov edi, 16
    mov esi, 0x405200
    mov eax, 157
    syscall
    cmp rax, -14
    jne failure

    mov edi, 16
    mov esi, 0x4040d0
    mov eax, 157
    syscall
    test rax, rax
    jne failure
    movabs rax, 0x72702d78756e6967
    cmp qword ptr [0x4040d0], rax
    jne failure
    cmp dword ptr [0x4040d8], 0x0065626f
    jne failure
    cmp dword ptr [0x4040dc], 0
    jne failure

    mov edi, 99
    mov eax, 157
    syscall
    cmp rax, -22
    jne failure

    mov dword ptr [0x4040c0], 0x74696e69
    mov byte ptr [0x4040c4], 0
    mov edi, 15
    mov esi, 0x4040c0
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
    mov esi, 0x405200
    mov rdx, -1
    xor eax, eax
    syscall
    cmp rax, -9
    jne failure

    xor edi, edi
    mov esi, 0x405200
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
    mov esi, 0x404200
    mov eax, 5
    syscall
    cmp rax, -9
    jne failure

    xor edi, edi
    mov esi, 0x5413
    mov edx, 0x404280
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
    mov esi, 0x405200
    mov eax, 115
    syscall
    cmp rax, 1
    jne failure

    mov dword ptr [0x404060], 0x12345678
    mov edi, 1
    mov esi, 0x405200
    mov eax, 115
    syscall
    cmp rax, -14
    jne failure
    cmp dword ptr [0x404060], 0x12345678
    jne failure

    mov edi, 1
    mov esi, 0x404060
    mov eax, 115
    syscall
    cmp rax, 1
    jne failure
    cmp dword ptr [0x404060], 0
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

    mov dword ptr [0x404060], 0x20080522
    mov dword ptr [0x404064], 0
    mov dword ptr [0x404068], 0x12345678
    mov edi, 0x404060
    mov esi, 0x405200
    mov eax, 125
    syscall
    cmp rax, -14
    jne failure
    cmp dword ptr [0x404068], 0x12345678
    jne failure

    mov edi, 0x404060
    mov esi, 0x404068
    mov eax, 125
    syscall
    test rax, rax
    jne failure
    mov eax, dword ptr [0x404068]
    or eax, dword ptr [0x40406c]
    or eax, dword ptr [0x404070]
    or eax, dword ptr [0x404074]
    or eax, dword ptr [0x404078]
    or eax, dword ptr [0x40407c]
    jne failure

    mov dword ptr [0x404060], 0
    mov edi, 0x404060
    xor esi, esi
    mov eax, 125
    syscall
    cmp rax, -22
    jne failure
    cmp dword ptr [0x404060], 0x20080522
    jne failure

    mov dword ptr [0x404064], 2
    mov eax, 125
    syscall
    cmp rax, -3
    jne failure

    mov dword ptr [0x404064], 0
    mov edi, 0x404060
    mov esi, 0x404068
    mov eax, 126
    syscall
    test rax, rax
    jne failure
    mov dword ptr [0x404068], 1
    mov eax, 126
    syscall
    cmp rax, -1
    jne failure
    mov dword ptr [0x404068], 0

    xor edi, edi
    xor esi, esi
    mov eax, 116
    syscall
    cmp rax, -1
    jne failure

    mov dword ptr [0x404060], 0x12345678
    mov dword ptr [0x404064], 0x12345678
    mov edi, 0x404060
    mov esi, 0x404064
    mov edx, 0x405200
    mov eax, 118
    syscall
    cmp rax, -14
    jne failure
    cmp dword ptr [0x404060], 0x12345678
    jne failure
    cmp dword ptr [0x404064], 0x12345678
    jne failure

    mov edi, 0x404060
    mov esi, 0x404064
    mov edx, 0x404068
    mov eax, 118
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x404060], 0
    jne failure
    cmp dword ptr [0x404064], 0
    jne failure
    cmp dword ptr [0x404068], 0
    jne failure

    mov dword ptr [0x404060], 0x12345678
    mov dword ptr [0x404064], 0x12345678
    mov dword ptr [0x404068], 0x12345678
    mov edi, 0x404060
    mov esi, 0x404064
    mov edx, 0x404068
    mov eax, 120
    syscall
    test rax, rax
    jne failure
    cmp dword ptr [0x404060], 0
    jne failure
    cmp dword ptr [0x404064], 0
    jne failure
    cmp dword ptr [0x404068], 0
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

    mov edi, 0x4041b0
    mov esi, 23
    mov eax, 273
    syscall
    cmp rax, -22
    jne failure

    mov edi, 0x4051f0
    mov esi, 24
    mov eax, 273
    syscall
    cmp rax, -14
    jne failure

    mov edi, 0x4041b0
    mov esi, 24
    mov eax, 273
    syscall
    test rax, rax
    jne failure

    mov edi, 2
    mov esi, 0x4041c8
    mov edx, 0x4041d0
    mov eax, 274
    syscall
    cmp rax, -3
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x4041c8], rax
    xor edi, edi
    mov esi, 0x4041c8
    mov edx, 0x4051fc
    mov eax, 274
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x4041c8], rax
    jne failure

    xor edi, edi
    mov esi, 0x4041c8
    mov edx, 0x4041d0
    mov eax, 274
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x4041c8], 0x4041b0
    jne failure
    cmp qword ptr [0x4041d0], 24
    jne failure

    mov edi, 0x405001
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    syscall
    cmp rax, -22
    jne failure

    mov edi, 0x405000
    mov esi, 4096
    mov edx, 4
    mov eax, 10
    syscall
    cmp rax, -22
    jne failure

    movabs rax, 0x0a212145424f5250
    mov qword ptr [0x405000], rax
    mov edi, 0x405000
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    syscall
    test rax, rax
    jne failure

    mov edi, 0x405000
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    syscall
    cmp rax, -22
    jne failure

    mov edi, 0x405000
    mov eax, 63
    syscall
    cmp rax, -14
    jne failure

    mov edi, 1
    mov esi, 0x405000
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x404280], rax
    mov qword ptr [0x404298], rax
    mov edi, 1
    mov esi, 0x5401
    mov edx, 0x404280
    mov eax, 16
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404280], 0
    jne failure
    cmp qword ptr [0x404298], 0
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
    mov qword ptr [0x404ff8], rax
    mov edi, 1
    mov esi, 0x404ff8
    mov eax, 228
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x404ff8], rax
    jne failure

    xor edi, edi
    mov esi, 0x404900
    mov eax, 228
    syscall
    cmp rax, -22
    jne failure

    mov edi, 1
    mov esi, 0x404900
    mov eax, 228
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404900], 0
    jl failure
    cmp qword ptr [0x404908], 1000000000
    jae failure

    mov edi, 1
    mov esi, 0x404910
    mov eax, 228
    syscall
    test rax, rax
    jne failure
    mov rax, qword ptr [0x404910]
    cmp rax, qword ptr [0x404900]
    jb failure
    ja clock_advanced
    mov rax, qword ptr [0x404918]
    cmp rax, qword ptr [0x404908]
    jb failure
clock_advanced:
    cmp qword ptr [0x404918], 1000000000
    jae failure

    mov edi, 4
    mov esi, 0x404920
    mov eax, 228
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404928], 1000000000
    jae failure

    mov edi, 2
    mov esi, 0x404950
    mov eax, 228
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404958], 1000000000
    jae failure
    mov edi, 3
    mov esi, 0x404960
    mov eax, 228
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404968], 1000000000
    jae failure

    xor edi, edi
    xor esi, esi
    mov eax, 229
    syscall
    cmp rax, -22
    jne failure

    mov edi, 1
    xor esi, esi
    mov eax, 229
    syscall
    test rax, rax
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x404ff8], rax
    mov edi, 4
    mov esi, 0x404ff8
    mov eax, 229
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x404ff8], rax
    jne failure

    mov edi, 1
    mov esi, 0x404930
    mov eax, 229
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404930], 0
    je resolution_subsecond
    cmp qword ptr [0x404930], 1
    jne failure
    cmp qword ptr [0x404938], 0
    jne failure
    jmp resolution_valid
resolution_subsecond:
    cmp qword ptr [0x404938], 0
    jbe failure
    cmp qword ptr [0x404938], 1000000000
    jae failure
resolution_valid:

    movabs rax, 0x1122334455667788
    mov qword ptr [0x404ff0], rax
    mov edi, 0x404ff0
    mov eax, 100
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x404ff0], rax
    jne failure
    xor edi, edi
    mov eax, 100
    syscall
    test rax, rax
    js failure
    mov edi, 0x404970
    mov eax, 100
    syscall
    test rax, rax
    js failure
    cmp qword ptr [0x404970], rax
    jne failure
    cmp qword ptr [0x404978], 0
    jne failure
    cmp qword ptr [0x404980], 0
    jne failure
    cmp qword ptr [0x404988], 0
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x404ff0], rax
    xor edi, edi
    mov esi, 0x404ff0
    mov eax, 36
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x404ff0], rax
    jne failure

    mov edi, 3
    mov esi, 0x404bc0
    mov eax, 36
    syscall
    cmp rax, -22
    jne failure

    xor edi, edi
    mov esi, 0x404bc0
    mov eax, 36
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404bc0], 0
    jne failure
    cmp qword ptr [0x404bd8], 0
    jne failure

    mov qword ptr [0x404be0], 1
    mov qword ptr [0x404be8], 0
    mov qword ptr [0x404bf0], 0
    mov qword ptr [0x404bf8], 0
    movabs rax, 0x1122334455667788
    mov qword ptr [0x404c00], rax
    xor edi, edi
    mov esi, 0x404be0
    mov edx, 0x404c00
    mov eax, 38
    syscall
    cmp rax, -38
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x404c00], rax
    jne failure

    mov qword ptr [0x404be0], 0
    xor edi, edi
    mov esi, 0x404be0
    mov edx, 0x404c00
    mov eax, 38
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404c00], 0
    jne failure
    cmp qword ptr [0x404c18], 0
    jne failure

    xor edi, edi
    mov esi, 0x405200
    xor edx, edx
    mov eax, 38
    syscall
    cmp rax, -14
    jne failure

    mov dword ptr [0x404c40], 42
    mov edi, 0x404c40
    mov esi, 2
    mov edx, 42
    mov eax, 202
    syscall
    cmp rax, -22
    jne failure

    mov edi, 0x404c41
    xor esi, esi
    mov eax, 202
    syscall
    cmp rax, -22
    jne failure
    mov edi, 0x405200
    mov eax, 202
    syscall
    cmp rax, -14
    jne failure

    mov edi, 0x404c40
    mov edx, 41
    mov eax, 202
    syscall
    cmp rax, -11
    jne failure
    mov esi, 128
    mov eax, 202
    syscall
    cmp rax, -11
    jne failure

    xor esi, esi
    mov edx, 42
    mov eax, 202
    syscall
    cmp rax, -38
    jne failure

    mov esi, 1
    mov edx, 1
    mov eax, 202
    syscall
    test rax, rax
    jne failure
    mov esi, 129
    mov eax, 202
    syscall
    test rax, rax
    jne failure

    movabs rax, 0x1122334455667788
    mov qword ptr [0x404ff0], rax
    mov qword ptr [0x404ff8], rax
    xor edi, edi
    mov esi, 0x404ff0
    mov eax, 98
    syscall
    cmp rax, -14
    jne failure
    movabs rax, 0x1122334455667788
    cmp qword ptr [0x404ff0], rax
    jne failure
    cmp qword ptr [0x404ff8], rax
    jne failure

    mov edi, 2
    mov esi, 0x404a00
    mov eax, 98
    syscall
    cmp rax, -22
    jne failure
    xor edi, edi
    mov eax, 98
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404a08], 1000000
    jae failure
    cmp qword ptr [0x404a10], 0
    jne failure
    cmp qword ptr [0x404a18], 0
    jne failure

    mov edi, 1
    mov esi, 0x404a90
    mov eax, 98
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404a98], 1000000
    jae failure

    mov edi, -1
    mov esi, 0x404b20
    mov eax, 98
    syscall
    test rax, rax
    jne failure
    cmp qword ptr [0x404b20], 0
    jne failure
    cmp qword ptr [0x404ba8], 0
    jne failure

    mov edi, 0x405200
    mov eax, 218
    syscall
    cmp rax, -14
    jne failure

    mov dword ptr [0x404190], 0x12345678
    mov edi, 0x404190
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

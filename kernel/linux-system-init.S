.intel_syntax noprefix

.section .text
.global ginux_linux_system_init
.type ginux_linux_system_init, @function
ginux_linux_system_init:
    mov eax, 39
    syscall
    cmp rax, 1
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lconsole]
    mov edx, 1
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r8, rax

    mov edi, -100
    lea rsi, [rip + .Ldev_path]
    mov edx, 0x10000
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 152
    mov eax, 217
    syscall
    cmp rax, 152
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 67], 0x6c6c756e
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 91], 0x6f72657a
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 115], 0x736e6f63
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 119], 0x00656c6f
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 147], 0x00797474
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lnull_path]
    mov edx, 2
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 4
    mov eax, 1
    syscall
    cmp rax, 4
    jne .Lfailure
    mov rdi, r9
    lea rsi, [rip + .Lstatus]
    mov edx, 4
    xor eax, eax
    syscall
    test rax, rax
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov dword ptr [rip + .Lstatus], -1
    mov edi, -100
    lea rsi, [rip + .Lzero_path]
    xor edx, edx
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 4
    xor eax, eax
    syscall
    cmp rax, 4
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus], 0
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Ltty_path]
    mov edx, 1
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    mov esi, 0x5413
    lea rdx, [rip + .Lstatus]
    mov eax, 16
    syscall
    test rax, rax
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov rdi, r8
    lea rsi, [rip + .Ldevfs_marker]
    mov edx, 12
    mov eax, 1
    syscall
    cmp rax, 12
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lproc_path]
    mov edx, 0x10000
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 80
    mov eax, 217
    syscall
    cmp rax, 72
    jne .Lfailure
    cmp byte ptr [rip + .Lstatus + 67], '1'
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lpid1_path]
    mov edx, 0x10000
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 256
    mov eax, 217
    syscall
    cmp rax, 208
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 67], 0x74617473
    jne .Lfailure
    cmp word ptr [rip + .Lstatus + 71], 0x7375
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 99], 0x6e756f6d
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 103], 0x666e6974
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 131], 0x5f646975
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 163], 0x5f646967
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 195], 0x67746573
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Luid_map_path]
    xor edx, edx
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 11
    xor eax, eax
    syscall
    cmp rax, 11
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus], 0x30312030
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 4], 0x30303030
    jne .Lfailure
    cmp byte ptr [rip + .Lstatus + 8], 0x20
    jne .Lfailure
    cmp word ptr [rip + .Lstatus + 9], 0x0a31
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lgid_map_path]
    xor edx, edx
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 11
    xor eax, eax
    syscall
    cmp rax, 11
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus], 0x30312030
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 4], 0x30303030
    jne .Lfailure
    cmp byte ptr [rip + .Lstatus + 8], 0x20
    jne .Lfailure
    cmp word ptr [rip + .Lstatus + 9], 0x0a31
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lsetgroups_path]
    xor edx, edx
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 5
    xor eax, eax
    syscall
    cmp rax, 5
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus], 0x796e6564
    jne .Lfailure
    cmp byte ptr [rip + .Lstatus + 4], 0x0a
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov rdi, r8
    lea rsi, [rip + .Luserns_marker]
    mov edx, 13
    mov eax, 1
    syscall
    cmp rax, 13
    jne .Lfailure

    mov edi, 1
    mov eax, 105
    syscall
    cmp rax, -1
    jne .Lfailure
    xor edi, edi
    xor esi, esi
    mov eax, 116
    syscall
    cmp rax, -1
    jne .Lfailure
    xor edi, edi
    mov eax, 161
    syscall
    cmp rax, -1
    jne .Lfailure
    xor edi, edi
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    mov eax, 165
    syscall
    cmp rax, -1
    jne .Lfailure
    xor edi, edi
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    mov eax, 169
    syscall
    cmp rax, -1
    jne .Lfailure
    mov edi, 3
    mov eax, 172
    syscall
    cmp rax, -1
    jne .Lfailure
    mov edi, 0x10000000
    mov eax, 272
    syscall
    cmp rax, -1
    jne .Lfailure
    xor edi, edi
    xor esi, esi
    xor edx, edx
    mov eax, 321
    syscall
    cmp rax, -1
    jne .Lfailure
    mov eax, 0x474e
    syscall
    cmp rax, -38
    jne .Lfailure
    mov eax, 102
    syscall
    test rax, rax
    jne .Lfailure
    mov eax, 104
    syscall
    test rax, rax
    jne .Lfailure
    mov rdi, r8
    lea rsi, [rip + .Lprivilege_marker]
    mov edx, 25
    mov eax, 1
    syscall
    cmp rax, 25
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lstatus_path]
    xor edx, edx
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 45
    xor eax, eax
    syscall
    cmp rax, 45
    jne .Lfailure
    mov rdi, r8
    lea rsi, [rip + .Lstatus]
    mov edx, 45
    mov eax, 1
    syscall
    cmp rax, 45
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lmountinfo_path]
    xor edx, edx
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 51
    xor eax, eax
    syscall
    cmp rax, 51
    jne .Lfailure
    mov rdi, r8
    lea rsi, [rip + .Lstatus]
    mov edx, 51
    mov eax, 1
    syscall
    cmp rax, 51
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Ltmp_path]
    mov edx, 0x10000
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 80
    mov eax, 217
    syscall
    cmp rax, 48
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Ltmp_file_path]
    mov edx, 0xc1
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax

    mov edi, -100
    lea rsi, [rip + .Ltmp_file_path]
    mov edx, 0xc1
    mov eax, 257
    syscall
    cmp rax, -17
    jne .Lfailure

    mov rdi, r9
    lea rsi, [rip + .Ltmp_payload]
    mov edx, 6
    mov eax, 1
    syscall
    cmp rax, 6
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Ltmp_path]
    mov edx, 0x10000
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 80
    mov eax, 217
    syscall
    cmp rax, 80
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 67], 0x756e6967
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 71], 0x65722d78
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 75], 0x00796461
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Ltmp_file_path]
    xor edx, edx
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 6
    xor eax, eax
    syscall
    cmp rax, 6
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus], 0x64616572
    jne .Lfailure
    cmp word ptr [rip + .Lstatus + 4], 0x0a79
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Ltmp_file_path]
    xor edx, edx
    mov eax, 263
    syscall
    cmp rax, -16
    jne .Lfailure

    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Ltmp_file_path]
    xor edx, edx
    mov eax, 263
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Ltmp_file_path]
    xor edx, edx
    mov eax, 257
    syscall
    cmp rax, -2
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Ltmp_path]
    mov edx, 0x10000
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 80
    mov eax, 217
    syscall
    cmp rax, 48
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov rdi, r8
    lea rsi, [rip + .Ltmp_marker]
    mov edx, 12
    mov eax, 1
    syscall
    cmp rax, 12
    jne .Lfailure

    mov eax, 57
    syscall
    test rax, rax
    js .Lfailure
    jz .Lfork_child

    mov r12, rax

    mov edi, -100
    lea rsi, [rip + .Lproc_path]
    mov edx, 0x10000
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 96
    mov eax, 217
    syscall
    cmp rax, 96
    jne .Lfailure
    cmp byte ptr [rip + .Lstatus + 91], '2'
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lpid2_path]
    mov edx, 0x10000
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 80
    mov eax, 217
    syscall
    cmp rax, 80
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 67], 0x74617473
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lpid2_status_path]
    xor edx, edx
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lstatus]
    mov edx, 44
    xor eax, eax
    syscall
    cmp rax, 44
    jne .Lfailure
    cmp byte ptr [rip + .Lstatus + 18], 'Z'
    jne .Lfailure
    cmp byte ptr [rip + .Lstatus + 34], '2'
    jne .Lfailure
    cmp byte ptr [rip + .Lstatus + 42], '1'
    jne .Lfailure
    mov rdi, r9
    mov eax, 3
    syscall
    test rax, rax
    jne .Lfailure

    mov rdi, r12
    xor esi, esi
    mov edx, 1
    xor r10d, r10d
    mov eax, 61
    syscall
    cmp rax, -22
    jne .Lfailure

    mov rdi, r12
    mov esi, 1
    xor edx, edx
    xor r10d, r10d
    mov eax, 61
    syscall
    cmp rax, -14
    jne .Lfailure

    mov dword ptr [rip + .Lstatus], -1
    mov rdi, r12
    lea rsi, [rip + .Lstatus]
    xor edx, edx
    xor r10d, r10d
    mov eax, 61
    syscall
    cmp rax, r12
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus], 0x2a00
    jne .Lfailure

    mov rdi, r12
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    mov eax, 61
    syscall
    cmp rax, -10
    jne .Lfailure

    mov edi, -100
    lea rsi, [rip + .Lpid2_status_path]
    xor edx, edx
    mov eax, 257
    syscall
    cmp rax, -2
    jne .Lfailure

    mov rdi, r8
    lea rsi, [rip + .Lprocfs_marker]
    mov edx, 13
    mov eax, 1
    syscall
    cmp rax, 13
    jne .Lfailure
    jmp .Lfork_complete

.Lfork_child:
    mov eax, 39
    syscall
    cmp rax, 2
    jne .Lfailure
    mov eax, 110
    syscall
    cmp rax, 1
    jne .Lfailure
    mov eax, 102
    syscall
    test rax, rax
    jne .Lfailure
    mov eax, 104
    syscall
    test rax, rax
    jne .Lfailure
    mov edi, 42
    mov eax, 60
    syscall
    ud2

.Lfork_complete:

    mov rdi, r8
    lea rsi, [rip + .Lready]
    mov edx, 21
    mov eax, 1
    syscall
    cmp rax, 21
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
    mov edi, 1
    mov eax, 231
    syscall
    ud2

.section .data
.align 8
.Lready:
    .ascii "GINUX_SYSTEM_INIT_V0\n"
.Lready_end:
.Lconsole:
    .asciz "/dev/console"
.Ldev_path:
    .asciz "/dev"
.Lnull_path:
    .asciz "/dev/null"
.Lzero_path:
    .asciz "/dev/zero"
.Ltty_path:
    .asciz "/dev/tty"
.Lstatus_path:
    .asciz "/proc/1/status"
.Lmountinfo_path:
    .asciz "/proc/1/mountinfo"
.Luid_map_path:
    .asciz "/proc/1/uid_map"
.Lgid_map_path:
    .asciz "/proc/1/gid_map"
.Lsetgroups_path:
    .asciz "/proc/1/setgroups"
.Lproc_path:
    .asciz "/proc"
.Lpid1_path:
    .asciz "/proc/1"
.Lpid2_path:
    .asciz "/proc/2"
.Lpid2_status_path:
    .asciz "/proc/2/status"
.Ltmp_path:
    .asciz "/tmp"
.Ltmp_file_path:
    .asciz "/tmp/ginux-ready"
.Ltmp_payload:
    .ascii "ready\n"
.Ltmp_marker:
    .ascii "TMPFS_READY\n"
.Ldevfs_marker:
    .ascii "DEVFS_READY\n"
.Lprocfs_marker:
    .ascii "PROCFS_READY\n"
.Luserns_marker:
    .ascii "USERNS_READY\n"
.Lprivilege_marker:
    .ascii "PRIVILEGE_BOUNDARY_READY\n"
.Lstatus:
    .space 256

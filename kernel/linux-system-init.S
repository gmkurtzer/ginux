.intel_syntax noprefix

.section .text
.global ginux_linux_system_init
.type ginux_linux_system_init, @function
ginux_linux_system_init:
    # This freestanding probe does not inspect argc/argv; reserve one mapped
    # word above its initial stack pointer for compact assertion subroutines.
    add rsp, 8
    mov eax, 39
    syscall
    call .Lrequire_one

    xor edi, edi
    xor esi, esi
    xor edx, edx
    mov eax, 154
    syscall
    call .Lrequire_zero

    mov edi, -100
    lea rsi, [rip + .Lconsole]
    mov edx, 1
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r15, rax

    mov edi, 1
    xor esi, esi
    mov eax, 434
    syscall
    test rax, rax
    js .Lfailure
    mov r12, rax
    mov rdi, rax
    mov rsi, r15
    xor edx, edx
    mov eax, 438
    syscall
    test rax, rax
    js .Lfailure
    mov r13, rax
    mov rdi, rax
    call .Lclose_descriptor
    mov rdi, r12
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    mov eax, 424
    syscall
    call .Lrequire_zero
    mov rdi, r12
    call .Lclose_descriptor

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
    call .Lclose_descriptor

    mov edi, 3
    xor esi, esi
    xor edx, edx
    mov eax, 139
    syscall
    cmp rax, 4
    jne .Lfailure
    mov edi, 2
    xor esi, esi
    lea rdx, [rip + .Lsysfs_name]
    mov eax, 139
    syscall
    call .Lrequire_zero
    cmp dword ptr [rip + .Lsysfs_name], 0x746f6f72
    jne .Lfailure
    cmp word ptr [rip + .Lsysfs_name + 4], 0x7366
    jne .Lfailure
    cmp byte ptr [rip + .Lsysfs_name + 6], 0
    jne .Lfailure
    mov edi, 1
    lea rsi, [rip + .Ltmpfs_name]
    xor edx, edx
    mov eax, 139
    syscall
    cmp rax, 2
    jne .Lfailure
    mov edi, 1
    lea rsi, [rip + .Lunknown_fs_name]
    xor edx, edx
    mov eax, 139
    syscall
    call .Lrequire_einval
    mov edi, 2
    mov esi, 4
    lea rdx, [rip + .Lsysfs_name]
    mov eax, 139
    syscall
    call .Lrequire_einval
    mov edi, 2
    xor esi, esi
    xor edx, edx
    mov eax, 139
    syscall
    call .Lrequire_efault
    mov edi, 1
    lea rsi, [rip + .Lustat]
    mov eax, 136
    syscall
    call .Lrequire_zero
    cmp qword ptr [rip + .Lustat], 1048576
    jne .Lfailure
    cmp qword ptr [rip + .Lustat + 8], 1048576
    jne .Lfailure
    cmp qword ptr [rip + .Lustat + 16], 0
    jne .Lfailure
    cmp qword ptr [rip + .Lustat + 24], 0
    jne .Lfailure
    mov edi, 2
    lea rsi, [rip + .Lustat]
    mov eax, 136
    syscall
    call .Lrequire_einval
    mov edi, 1
    xor esi, esi
    mov eax, 136
    syscall
    call .Lrequire_efault

    mov edi, 1
    mov esi, 1
    xor edx, edx
    lea r10, [rip + .Lsocket_pair]
    mov eax, 53
    syscall
    call .Lrequire_zero
    mov edi, 2
    mov esi, 1
    xor edx, edx
    lea r10, [rip + .Lsocket_pair]
    mov eax, 53
    syscall
    cmp rax, -97
    jne .Lfailure
    mov edi, 1
    mov esi, 1
    mov edx, 1
    lea r10, [rip + .Lsocket_pair]
    mov eax, 53
    syscall
    cmp rax, -93
    jne .Lfailure
    mov edi, 1
    mov esi, 1
    xor edx, edx
    mov r10d, 1
    mov eax, 53
    syscall
    call .Lrequire_efault
    mov dword ptr [rip + .Lstatus + 16], 12
    mov edi, dword ptr [rip + .Lsocket_pair]
    mov esi, 1
    mov edx, 17
    lea r10, [rip + .Lstatus]
    lea r8, [rip + .Lstatus + 16]
    mov eax, 55
    syscall
    call .Lrequire_zero
    mov eax, 39
    syscall
    cmp dword ptr [rip + .Lstatus], eax
    jne .Lfailure
    cmp qword ptr [rip + .Lstatus + 4], 0
    jne .Lfailure
    mov dword ptr [rip + .Lstatus + 16], 16
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lstatus]
    lea rdx, [rip + .Lstatus + 16]
    mov eax, 51
    syscall
    call .Lrequire_zero
    cmp word ptr [rip + .Lstatus], 1
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 16], 2
    jne .Lfailure
    mov dword ptr [rip + .Lstatus + 16], 16
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lstatus]
    lea rdx, [rip + .Lstatus + 16]
    mov eax, 52
    syscall
    call .Lrequire_zero
    cmp word ptr [rip + .Lstatus], 1
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 16], 2
    jne .Lfailure
    mov dword ptr [rip + .Lstatus + 16], 16
    mov edi, dword ptr [rip + .Lsocket_pair]
    mov esi, 1
    lea rdx, [rip + .Lstatus + 16]
    mov eax, 52
    syscall
    call .Lrequire_efault
    lea rax, [rip + .Lmmsg_send_iov]
    mov qword ptr [rip + .Lmmsg_send + 16], rax
    mov qword ptr [rip + .Lmmsg_send + 24], 1
    lea rax, [rip + .Lmmsg_payload]
    mov qword ptr [rip + .Lmmsg_send_iov], rax
    mov qword ptr [rip + .Lmmsg_send_iov + 8], 5
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_send]
    mov edx, 1
    xor r10d, r10d
    mov eax, 307
    syscall
    call .Lrequire_one
    cmp dword ptr [rip + .Lmmsg_send + 56], 5
    jne .Lfailure
    lea rax, [rip + .Lmmsg_recv_iov]
    mov qword ptr [rip + .Lmmsg_recv + 16], rax
    mov qword ptr [rip + .Lmmsg_recv + 24], 1
    lea rax, [rip + .Lmmsg_output]
    mov qword ptr [rip + .Lmmsg_recv_iov], rax
    mov qword ptr [rip + .Lmmsg_recv_iov + 8], 5
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_payload]
    mov edx, 5
    mov r10d, 1
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 44
    syscall
    call .Lrequire_einval
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_output]
    mov edx, 5
    mov r10d, 1
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 45
    syscall
    call .Lrequire_einval
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_payload]
    mov edx, 2
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 44
    syscall
    cmp rax, 2
    jne .Lfailure
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_payload + 2]
    mov edx, 3
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 44
    syscall
    cmp rax, 3
    jne .Lfailure
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_output]
    mov edx, 5
    mov r10d, 0x102
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 45
    syscall
    cmp rax, 5
    jne .Lfailure
    cmp dword ptr [rip + .Lmmsg_output], 0x67736d6d
    jne .Lfailure
    cmp byte ptr [rip + .Lmmsg_output + 4], '!'
    jne .Lfailure
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_output]
    mov edx, 5
    mov r10d, 0x100
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 45
    syscall
    cmp rax, 5
    jne .Lfailure
    cmp dword ptr [rip + .Lmmsg_output], 0x67736d6d
    jne .Lfailure
    cmp byte ptr [rip + .Lmmsg_output + 4], '!'
    jne .Lfailure
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_send]
    mov edx, 65
    xor r10d, r10d
    mov eax, 307
    syscall
    call .Lrequire_einval
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_recv]
    mov edx, 65
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 299
    syscall
    call .Lrequire_einval
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_send]
    xor edx, edx
    mov eax, 46
    syscall
    cmp rax, 5
    jne .Lfailure
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_recv]
    mov edx, 0x102
    mov eax, 47
    syscall
    cmp rax, 5
    jne .Lfailure
    cmp dword ptr [rip + .Lmmsg_output], 0x67736d6d
    jne .Lfailure
    cmp byte ptr [rip + .Lmmsg_output + 4], '!'
    jne .Lfailure
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_recv]
    mov edx, 0x100
    mov eax, 47
    syscall
    cmp rax, 5
    jne .Lfailure
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_send]
    mov edx, 1
    mov eax, 46
    syscall
    call .Lrequire_einval
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_recv]
    mov edx, 1
    mov eax, 47
    syscall
    call .Lrequire_einval
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_recv]
    mov edx, 1
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 299
    syscall
    call .Lrequire_one
    cmp dword ptr [rip + .Lmmsg_recv + 56], 5
    jne .Lfailure
    cmp dword ptr [rip + .Lmmsg_output], 0x67736d6d
    jne .Lfailure
    cmp byte ptr [rip + .Lmmsg_output + 4], '!'
    jne .Lfailure

    # Pass two references to a socket over AF_UNIX and request close-on-exec.
    lea rax, [rip + .Lright_payload]
    mov qword ptr [rip + .Lmmsg_send_iov], rax
    mov qword ptr [rip + .Lmmsg_send_iov + 8], 1
    lea rax, [rip + .Lright_control]
    mov qword ptr [rip + .Lmmsg_send + 32], rax
    mov qword ptr [rip + .Lmmsg_send + 40], 24
    mov qword ptr [rip + .Lright_control], 24
    mov dword ptr [rip + .Lright_control + 8], 1
    mov dword ptr [rip + .Lright_control + 12], 1
    mov edi, 2
    mov esi, 2
    xor edx, edx
    mov eax, 41
    syscall
    test rax, rax
    js .Lfailure
    mov dword ptr [rip + .Lright_source], eax
    mov dword ptr [rip + .Lright_control + 16], eax
    mov dword ptr [rip + .Lright_control + 20], eax
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_send]
    xor edx, edx
    mov eax, 46
    syscall
    call .Lrequire_one
    mov edi, dword ptr [rip + .Lright_source]
    call .Lclose_descriptor
    lea rax, [rip + .Lright_output]
    mov qword ptr [rip + .Lmmsg_recv_iov], rax
    mov qword ptr [rip + .Lmmsg_recv_iov + 8], 1
    lea rax, [rip + .Lright_output_control]
    mov qword ptr [rip + .Lmmsg_recv + 32], rax
    mov qword ptr [rip + .Lmmsg_recv + 40], 24
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_recv]
    mov edx, 0x40000000
    mov eax, 47
    syscall
    call .Lrequire_one
    cmp byte ptr [rip + .Lright_output], 'R'
    jne .Lfailure
    cmp qword ptr [rip + .Lmmsg_recv + 40], 24
    jne .Lfailure
    cmp dword ptr [rip + .Lmmsg_recv + 48], 0
    jne .Lfailure
    cmp qword ptr [rip + .Lright_output_control], 24
    jne .Lfailure
    cmp dword ptr [rip + .Lright_output_control + 8], 1
    jne .Lfailure
    cmp dword ptr [rip + .Lright_output_control + 12], 1
    jne .Lfailure
    mov edi, dword ptr [rip + .Lright_output_control + 16]
    mov esi, 1
    xor edx, edx
    mov eax, 72
    syscall
    call .Lrequire_one
    mov dword ptr [rip + .Lstatus], 0
    mov dword ptr [rip + .Lstatus + 16], 4
    mov edi, dword ptr [rip + .Lright_output_control + 16]
    mov esi, 1
    mov edx, 3
    lea r10, [rip + .Lstatus]
    lea r8, [rip + .Lstatus + 16]
    mov eax, 55
    syscall
    call .Lrequire_zero
    cmp dword ptr [rip + .Lstatus], 2
    jne .Lfailure
    mov edi, dword ptr [rip + .Lright_output_control + 16]
    call .Lclose_descriptor
    mov edi, dword ptr [rip + .Lright_output_control + 20]
    cmp edi, dword ptr [rip + .Lright_output_control + 16]
    je .Lfailure
    call .Lclose_descriptor

    # A short control buffer consumes the rights and reports MSG_CTRUNC.
    mov qword ptr [rip + .Lright_control], 20
    mov eax, dword ptr [rip + .Lsocket_pair]
    mov dword ptr [rip + .Lright_control + 16], eax
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_send]
    xor edx, edx
    mov eax, 46
    syscall
    call .Lrequire_one
    mov qword ptr [rip + .Lmmsg_recv + 40], 8
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_recv]
    xor edx, edx
    mov eax, 47
    syscall
    call .Lrequire_one
    cmp qword ptr [rip + .Lmmsg_recv + 40], 0
    jne .Lfailure
    test dword ptr [rip + .Lmmsg_recv + 48], 8
    jz .Lfailure
    mov qword ptr [rip + .Lright_control], 19
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_send]
    xor edx, edx
    mov eax, 46
    syscall
    call .Lrequire_einval
    mov qword ptr [rip + .Lright_control], 20
    mov dword ptr [rip + .Lright_control + 16], -1
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_send]
    xor edx, edx
    mov eax, 46
    syscall
    cmp rax, -9
    jne .Lfailure
    mov eax, dword ptr [rip + .Lsocket_pair]
    mov dword ptr [rip + .Lright_control + 16], eax

    # Consuming payload without recvmsg discards its attached right.
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_send]
    xor edx, edx
    mov eax, 46
    syscall
    call .Lrequire_one
    mov qword ptr [rip + .Lmmsg_recv + 32], 0
    mov qword ptr [rip + .Lmmsg_recv + 40], 24
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_recv]
    xor edx, edx
    mov eax, 47
    syscall
    call .Lrequire_efault
    lea rax, [rip + .Lright_output_control]
    mov qword ptr [rip + .Lmmsg_recv + 32], rax
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lright_output]
    mov edx, 1
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 45
    syscall
    call .Lrequire_one
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lright_payload]
    mov edx, 1
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 44
    syscall
    call .Lrequire_one
    mov qword ptr [rip + .Lmmsg_recv + 40], 24
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_recv]
    xor edx, edx
    mov eax, 47
    syscall
    call .Lrequire_one
    cmp qword ptr [rip + .Lmmsg_recv + 40], 0
    jne .Lfailure
    cmp dword ptr [rip + .Lmmsg_recv + 48], 0
    jne .Lfailure

    # SO_PASSCRED attaches the sending Linux process identity to local data.
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    mov esi, 1
    mov edx, 16
    call .Lset_socket_boolean
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lright_payload]
    mov edx, 1
    mov eax, 1
    syscall
    call .Lrequire_one
    lea rax, [rip + .Lright_output_control]
    mov qword ptr [rip + .Lmmsg_recv + 32], rax
    mov qword ptr [rip + .Lmmsg_recv + 40], 32
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_recv]
    xor edx, edx
    mov eax, 47
    syscall
    call .Lrequire_one
    cmp qword ptr [rip + .Lmmsg_recv + 40], 32
    jne .Lfailure
    cmp qword ptr [rip + .Lright_output_control], 28
    jne .Lfailure
    cmp dword ptr [rip + .Lright_output_control + 12], 2
    jne .Lfailure
    mov eax, 39
    syscall
    cmp dword ptr [rip + .Lright_output_control + 16], eax
    jne .Lfailure
    cmp qword ptr [rip + .Lright_output_control + 20], 0
    jne .Lfailure

    mov edi, dword ptr [rip + .Lsocket_pair]
    mov esi, 2
    mov eax, 48
    syscall
    call .Lrequire_zero

    mov eax, dword ptr [rip + .Lsocket_pair]
    mov dword ptr [rip + .Lstatus], eax
    mov word ptr [rip + .Lstatus + 4], 5
    mov word ptr [rip + .Lstatus + 6], 0
    lea rdi, [rip + .Lstatus]
    mov esi, 1
    xor edx, edx
    mov eax, 7
    syscall
    call .Lrequire_one
    cmp word ptr [rip + .Lstatus + 6], 1
    jne .Lfailure

    # A broken stream send raises SIGPIPE unless MSG_NOSIGNAL is present.
    mov qword ptr [rip + .Lsignal_action], 0
    mov qword ptr [rip + .Lsignal_action + 8], 0
    mov qword ptr [rip + .Lsignal_action + 16], 0
    mov qword ptr [rip + .Lsignal_action + 24], 0
    mov edi, 13
    lea rsi, [rip + .Lsignal_action]
    lea rdx, [rip + .Lsignal_old_pipe]
    mov r10d, 8
    mov eax, 13
    syscall
    call .Lrequire_zero
    mov qword ptr [rip + .Lsignal_set], 0x1000
    xor edi, edi
    lea rsi, [rip + .Lsignal_set]
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    call .Lrequire_zero

    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_payload]
    mov edx, 5
    mov r10d, 0x4000
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 44
    syscall
    cmp rax, -32
    jne .Lfailure
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_send]
    mov edx, 0x4000
    mov eax, 46
    syscall
    cmp rax, -32
    jne .Lfailure
    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_send]
    mov edx, 1
    mov r10d, 0x4000
    mov eax, 307
    syscall
    cmp rax, -32
    jne .Lfailure
    lea rdi, [rip + .Lstatus]
    mov esi, 8
    mov eax, 127
    syscall
    call .Lrequire_zero
    cmp qword ptr [rip + .Lstatus], 0
    jne .Lfailure

    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_payload]
    mov edx, 5
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 44
    syscall
    cmp rax, -32
    jne .Lfailure
    lea rdi, [rip + .Lsignal_set]
    lea rsi, [rip + .Lstatus]
    xor edx, edx
    mov r10d, 8
    mov eax, 128
    syscall
    cmp rax, 13
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus], 13
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 8], 0x80
    jne .Lfailure

    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_send]
    xor edx, edx
    mov eax, 46
    syscall
    cmp rax, -32
    jne .Lfailure
    lea rdi, [rip + .Lsignal_set]
    lea rsi, [rip + .Lstatus]
    xor edx, edx
    mov r10d, 8
    mov eax, 128
    syscall
    cmp rax, 13
    jne .Lfailure

    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_send]
    mov edx, 1
    xor r10d, r10d
    mov eax, 307
    syscall
    cmp rax, -32
    jne .Lfailure
    lea rdi, [rip + .Lsignal_set]
    lea rsi, [rip + .Lstatus]
    xor edx, edx
    mov r10d, 8
    mov eax, 128
    syscall
    cmp rax, 13
    jne .Lfailure
    lea rdi, [rip + .Lstatus]
    mov esi, 8
    mov eax, 127
    syscall
    call .Lrequire_zero
    cmp qword ptr [rip + .Lstatus], 0
    jne .Lfailure
    mov edi, 1
    lea rsi, [rip + .Lsignal_set]
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    call .Lrequire_zero
    mov edi, 13
    lea rsi, [rip + .Lsignal_old_pipe]
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    call .Lrequire_zero

    mov edi, dword ptr [rip + .Lsocket_pair]
    lea rsi, [rip + .Lmmsg_output]
    mov edx, 5
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 45
    syscall
    call .Lrequire_zero
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_output]
    mov edx, 5
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 45
    syscall
    call .Lrequire_zero
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    mov rax, 0x100000000
    or rdi, rax
    mov esi, 1
    mov eax, 48
    syscall
    call .Lrequire_zero
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    lea rsi, [rip + .Lmmsg_payload]
    mov edx, 5
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 44
    syscall
    cmp rax, -32
    jne .Lfailure
    mov edi, dword ptr [rip + .Lsocket_pair]
    mov esi, 3
    mov eax, 48
    syscall
    call .Lrequire_einval
    mov edi, 1
    xor esi, esi
    mov eax, 48
    syscall
    cmp rax, -88
    jne .Lfailure
    mov edi, 9999
    xor esi, esi
    mov eax, 48
    syscall
    cmp rax, -9
    jne .Lfailure
    mov edi, dword ptr [rip + .Lsocket_pair]
    call .Lclose_descriptor
    mov edi, dword ptr [rip + .Lsocket_pair + 4]
    call .Lclose_descriptor

    # UDP truncation copies the available prefix, reports the full datagram
    # length under MSG_TRUNC, and preserves a peeked DNS response.
    mov edi, 2
    mov esi, 2
    xor edx, edx
    mov eax, 41
    syscall
    test rax, rax
    js .Lfailure
    mov dword ptr [rip + .Ldns_socket], eax
    mov edi, eax
    lea rsi, [rip + .Ldns_address]
    mov edx, 16
    mov eax, 42
    syscall
    call .Lrequire_zero
    mov edi, dword ptr [rip + .Ldns_socket]
    lea rsi, [rip + .Ldns_query]
    mov edx, 29
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 44
    syscall
    cmp rax, 29
    jne .Lfailure
    mov ebx, 1024
.Ldns_peek:
    mov edi, dword ptr [rip + .Ldns_socket]
    lea rsi, [rip + .Lmmsg_output]
    mov edx, 4
    mov r10d, 0x62
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 45
    syscall
    cmp rax, -11
    jne .Ldns_received
    dec ebx
    jnz .Ldns_peek
    jmp .Ldns_close
.Ldns_received:
    cmp rax, 12
    jb .Lfailure
    cmp word ptr [rip + .Lmmsg_output], 0x3412
    jne .Lfailure
    mov edi, dword ptr [rip + .Ldns_socket]
    lea rsi, [rip + .Lmmsg_output]
    mov edx, 4
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 45
    syscall
    cmp rax, 4
    jne .Lfailure
    cmp word ptr [rip + .Lmmsg_output], 0x3412
    jne .Lfailure
    mov edi, dword ptr [rip + .Ldns_socket]
    lea rsi, [rip + .Ldns_query]
    mov edx, 29
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 44
    syscall
    cmp rax, 29
    jne .Lfailure
    lea rax, [rip + .Lmmsg_output]
    mov qword ptr [rip + .Lmmsg_recv_iov], rax
    mov qword ptr [rip + .Lmmsg_recv_iov + 8], 4
    mov dword ptr [rip + .Lmmsg_recv + 48], 0
    mov edi, dword ptr [rip + .Ldns_socket]
    lea rsi, [rip + .Lmmsg_recv]
    xor edx, edx
    mov eax, 47
    syscall
    cmp rax, 4
    jne .Lfailure
    cmp dword ptr [rip + .Lmmsg_recv + 48], 0x20
    jne .Lfailure
    cmp word ptr [rip + .Lmmsg_output], 0x3412
    jne .Lfailure
.Ldns_close:
    mov edi, dword ptr [rip + .Ldns_socket]
    call .Lclose_descriptor

    mov edi, 2
    mov esi, 2
    xor edx, edx
    mov eax, 41
    syscall
    test rax, rax
    js .Lfailure
    mov dword ptr [rip + .Lipv4_socket], eax
    mov dword ptr [rip + .Lstatus + 16], 16
    mov edi, eax
    lea rsi, [rip + .Lstatus]
    lea rdx, [rip + .Lstatus + 16]
    mov eax, 52
    syscall
    cmp rax, -107
    jne .Lfailure
    mov edi, dword ptr [rip + .Lipv4_socket]
    lea rsi, [rip + .Lipv4_peer]
    mov edx, 16
    mov eax, 42
    syscall
    call .Lrequire_zero
    mov dword ptr [rip + .Lstatus + 16], 1
    mov edi, dword ptr [rip + .Lipv4_socket]
    lea rsi, [rip + .Lstatus]
    lea rdx, [rip + .Lstatus + 16]
    mov eax, 52
    syscall
    call .Lrequire_zero
    cmp byte ptr [rip + .Lstatus], 2
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 16], 16
    jne .Lfailure
    mov dword ptr [rip + .Lstatus + 16], 16
    mov edi, dword ptr [rip + .Lipv4_socket]
    mov rax, 0x100000000
    or rdi, rax
    lea rsi, [rip + .Lstatus]
    lea rdx, [rip + .Lstatus + 16]
    mov eax, 52
    syscall
    call .Lrequire_zero
    cmp word ptr [rip + .Lstatus], 2
    jne .Lfailure
    cmp word ptr [rip + .Lstatus + 2], 0x3500
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 4], 0x0202000a
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 16], 16
    jne .Lfailure
    mov edi, dword ptr [rip + .Lipv4_socket]
    lea rsi, [rip + .Lstatus]
    xor edx, edx
    mov eax, 52
    syscall
    call .Lrequire_efault
    mov dword ptr [rip + .Lstatus + 16], 16
    mov edi, dword ptr [rip + .Lipv4_socket]
    mov esi, 1
    lea rdx, [rip + .Lstatus + 16]
    mov eax, 52
    syscall
    call .Lrequire_efault
    mov edi, 1
    lea rsi, [rip + .Lstatus]
    lea rdx, [rip + .Lstatus + 16]
    mov eax, 52
    syscall
    cmp rax, -88
    jne .Lfailure
    mov edi, 9999
    lea rsi, [rip + .Lstatus]
    lea rdx, [rip + .Lstatus + 16]
    mov eax, 52
    syscall
    cmp rax, -9
    jne .Lfailure
    mov edi, dword ptr [rip + .Lipv4_socket]
    call .Lclose_descriptor

    mov edi, 2
    mov esi, 1
    xor edx, edx
    mov eax, 41
    syscall
    test rax, rax
    js .Lfailure
    mov dword ptr [rip + .Ltcp_listener], eax
    mov edi, eax
    lea rsi, [rip + .Ltcp_bind]
    mov edx, 16
    mov eax, 49
    syscall
    call .Lrequire_zero
    mov edi, dword ptr [rip + .Ltcp_listener]
    mov esi, 2
    mov eax, 50
    syscall
    call .Lrequire_zero
    mov dword ptr [rip + .Ltcp_address_length], 16
    mov edi, dword ptr [rip + .Ltcp_listener]
    lea rsi, [rip + .Ltcp_address]
    lea rdx, [rip + .Ltcp_address_length]
    mov eax, 51
    syscall
    call .Lrequire_zero
    cmp word ptr [rip + .Ltcp_address], 2
    jne .Lfailure
    cmp word ptr [rip + .Ltcp_address + 2], 0xa046
    jne .Lfailure
    mov edi, 2
    mov esi, 1
    xor edx, edx
    mov eax, 41
    syscall
    test rax, rax
    js .Lfailure
    mov dword ptr [rip + .Ltcp_client], eax
    mov edi, eax
    lea rsi, [rip + .Ltcp_address]
    mov edx, 16
    mov eax, 42
    syscall
    call .Lrequire_zero
    mov edi, 2
    mov esi, 1
    xor edx, edx
    mov eax, 41
    syscall
    test rax, rax
    js .Lfailure
    mov dword ptr [rip + .Ltcp_client_second], eax
    mov edi, eax
    lea rsi, [rip + .Ltcp_address]
    mov edx, 16
    mov eax, 42
    syscall
    call .Lrequire_zero
    mov dword ptr [rip + .Ltcp_peer_length], 16
    mov edi, dword ptr [rip + .Ltcp_listener]
    lea rsi, [rip + .Ltcp_peer]
    lea rdx, [rip + .Ltcp_peer_length]
    mov r10d, 0x80800
    mov eax, 288
    syscall
    test rax, rax
    js .Lfailure
    mov dword ptr [rip + .Ltcp_accepted], eax
    cmp word ptr [rip + .Ltcp_peer], 2
    jne .Lfailure
    cmp dword ptr [rip + .Ltcp_peer_length], 16
    jne .Lfailure
    mov edi, eax
    mov esi, 1
    xor edx, edx
    mov eax, 72
    syscall
    call .Lrequire_one
    mov edi, dword ptr [rip + .Ltcp_accepted]
    mov esi, 3
    xor edx, edx
    mov eax, 72
    syscall
    test rax, 0x800
    jz .Lfailure
    mov edi, dword ptr [rip + .Ltcp_client]
    lea rsi, [rip + .Ltcp_payload]
    mov edx, 2
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 44
    syscall
    cmp rax, 2
    jne .Lfailure
    mov edi, dword ptr [rip + .Ltcp_client]
    lea rsi, [rip + .Ltcp_payload + 2]
    mov edx, 2
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 44
    syscall
    cmp rax, 2
    jne .Lfailure
    mov edi, dword ptr [rip + .Ltcp_accepted]
    lea rsi, [rip + .Ltcp_output]
    mov edx, 4
    mov r10d, 0x102
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 45
    syscall
    cmp rax, 4
    jne .Lfailure
    cmp dword ptr [rip + .Ltcp_output], 0x21706374
    jne .Lfailure
    mov edi, dword ptr [rip + .Ltcp_accepted]
    lea rsi, [rip + .Ltcp_output]
    mov edx, 4
    mov r10d, 0x100
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 45
    syscall
    cmp rax, 4
    jne .Lfailure
    cmp dword ptr [rip + .Ltcp_output], 0x21706374
    jne .Lfailure
    mov edi, dword ptr [rip + .Ltcp_client]
    mov esi, 1
    mov eax, 48
    syscall
    call .Lrequire_zero
    mov eax, dword ptr [rip + .Ltcp_client]
    mov dword ptr [rip + .Lstatus], eax
    mov word ptr [rip + .Lstatus + 4], 4
    mov word ptr [rip + .Lstatus + 6], 0
    lea rdi, [rip + .Lstatus]
    mov esi, 1
    xor edx, edx
    mov eax, 7
    syscall
    call .Lrequire_zero
    cmp word ptr [rip + .Lstatus + 6], 0
    jne .Lfailure
    mov edi, dword ptr [rip + .Ltcp_client]
    lea rsi, [rip + .Ltcp_payload]
    mov edx, 4
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 44
    syscall
    cmp rax, -32
    jne .Lfailure
    mov edi, dword ptr [rip + .Ltcp_accepted]
    lea rsi, [rip + .Ltcp_output]
    mov edx, 4
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 45
    syscall
    call .Lrequire_zero
    mov eax, dword ptr [rip + .Ltcp_accepted]
    mov dword ptr [rip + .Lstatus], eax
    mov word ptr [rip + .Lstatus + 4], 1
    mov word ptr [rip + .Lstatus + 6], 0
    lea rdi, [rip + .Lstatus]
    mov esi, 1
    xor edx, edx
    mov eax, 7
    syscall
    call .Lrequire_one
    cmp word ptr [rip + .Lstatus + 6], 1
    jne .Lfailure
    mov edi, dword ptr [rip + .Ltcp_accepted]
    lea rsi, [rip + .Ltcp_payload]
    mov edx, 4
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 44
    syscall
    cmp rax, 4
    jne .Lfailure
    mov edi, dword ptr [rip + .Ltcp_client]
    lea rsi, [rip + .Ltcp_output]
    mov edx, 4
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 45
    syscall
    cmp rax, 4
    jne .Lfailure
    mov edi, dword ptr [rip + .Ltcp_client]
    xor esi, esi
    mov eax, 48
    syscall
    call .Lrequire_zero
    mov edi, dword ptr [rip + .Ltcp_client]
    lea rsi, [rip + .Ltcp_output]
    mov edx, 4
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 45
    syscall
    call .Lrequire_zero
    mov edi, dword ptr [rip + .Ltcp_client]
    call .Lclose_descriptor
    mov edi, dword ptr [rip + .Ltcp_accepted]
    call .Lclose_descriptor
    mov eax, dword ptr [rip + .Ltcp_client_second]
    mov dword ptr [rip + .Ltcp_client], eax
    mov edi, dword ptr [rip + .Ltcp_listener]
    xor esi, esi
    xor edx, edx
    mov eax, 43
    syscall
    test rax, rax
    js .Lfailure
    mov dword ptr [rip + .Ltcp_accepted], eax
    mov edi, eax
    mov esi, 1
    xor edx, edx
    mov eax, 72
    syscall
    call .Lrequire_zero
    mov edi, dword ptr [rip + .Ltcp_accepted]
    mov esi, 3
    xor edx, edx
    mov eax, 72
    syscall
    test rax, 0x800
    jnz .Lfailure
    mov edi, dword ptr [rip + .Ltcp_client]
    call .Lclose_descriptor
    mov edi, dword ptr [rip + .Ltcp_accepted]
    call .Lclose_descriptor
    mov edi, dword ptr [rip + .Ltcp_listener]
    call .Lclose_descriptor

    xor edi, edi
    mov esi, 4096
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    test rax, rax
    js .Lfailure
    mov r12, rax
    mov rdi, rax
    mov esi, 4096
    mov edx, 1
    mov r10, -1
    mov eax, 329
    syscall
    call .Lrequire_zero
    mov rdi, r12
    mov esi, 4096
    mov edx, 3
    xor r10d, r10d
    mov eax, 329
    syscall
    call .Lrequire_einval
    mov rdi, r12
    mov esi, 4096
    mov eax, 11
    syscall
    call .Lrequire_zero
    xor edi, edi
    xor esi, esi
    mov eax, 330
    syscall
    cmp rax, -28
    jne .Lfailure
    xor edi, edi
    mov eax, 331
    syscall
    call .Lrequire_einval

    xor edi, edi
    mov esi, 4096
    mov edx, 3
    mov r10d, 0x22
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    test rax, rax
    js .Lfailure
    mov r12, rax
    mov rdi, rax
    mov esi, 1
    mov edx, 1
    mov eax, 462
    syscall
    call .Lrequire_einval
    mov rdi, r12
    mov esi, 1
    xor edx, edx
    mov eax, 462
    syscall
    call .Lrequire_zero
    mov rdi, r12
    mov esi, 4096
    xor edx, edx
    mov eax, 462
    syscall
    call .Lrequire_zero
    mov rdi, r12
    mov esi, 4096
    mov edx, 1
    mov eax, 10
    syscall
    cmp rax, -1
    jne .Lfailure
    mov rdi, r12
    mov esi, 4096
    mov eax, 11
    syscall
    cmp rax, -1
    jne .Lfailure
    mov rdi, r12
    mov esi, 4096
    mov edx, 3
    mov r10d, 0x32
    mov r8, -1
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, -1
    jne .Lfailure
    mov edi, 0x1000
    mov esi, 1
    xor edx, edx
    mov eax, 462
    syscall
    cmp rax, -12
    jne .Lfailure

    lea rdi, [rip + .Lpipe_pair]
    mov eax, 22
    syscall
    call .Lrequire_zero
    mov edi, dword ptr [rip + .Lpipe_pair]
    xor esi, esi
    mov edx, dword ptr [rip + .Lpipe_pair + 4]
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 275
    syscall
    call .Lrequire_zero
    mov edi, dword ptr [rip + .Lpipe_pair]
    mov esi, dword ptr [rip + .Lpipe_pair + 4]
    xor edx, edx
    xor r10d, r10d
    mov eax, 276
    syscall
    call .Lrequire_zero
    lea rax, [rip + .Lvmsplice_payload]
    mov qword ptr [rip + .Lvmsplice_iov], rax
    mov qword ptr [rip + .Lvmsplice_iov + 8], 8
    mov edi, dword ptr [rip + .Lpipe_pair + 4]
    lea rsi, [rip + .Lvmsplice_iov]
    mov edx, 1
    xor r10d, r10d
    mov eax, 278
    syscall
    cmp rax, 8
    jne .Lfailure
    mov edi, dword ptr [rip + .Lpipe_pair]
    lea rsi, [rip + .Lvmsplice_output]
    mov edx, 8
    xor eax, eax
    syscall
    cmp rax, 8
    jne .Lfailure
    mov rax, 0x216563696c707376
    cmp qword ptr [rip + .Lvmsplice_output], rax
    jne .Lfailure
    mov edi, dword ptr [rip + .Lpipe_pair]
    call .Lclose_descriptor
    mov edi, dword ptr [rip + .Lpipe_pair + 4]
    call .Lclose_descriptor

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
    call .Lrequire_zero
    mov rdi, r9
    call .Lclose_descriptor

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
    call .Lclose_descriptor

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
    call .Lrequire_zero
    mov rdi, r9
    call .Lclose_descriptor

    mov edi, 1
    mov esi, 1
    xor edx, edx
    mov eax, 41
    syscall
    cmp rax, -97
    jne .Lfailure
    mov edi, 2
    mov esi, 3
    xor edx, edx
    mov eax, 41
    syscall
    call .Lrequire_einval
    mov edi, 2
    mov esi, 1
    mov edx, 17
    mov eax, 41
    syscall
    cmp rax, -93
    jne .Lfailure
    mov edi, 2
    mov esi, 0x80801
    xor edx, edx
    mov eax, 41
    syscall
    cmp rax, 4
    jne .Lfailure
    mov edi, 4
    mov esi, 1
    mov edx, 2
    call .Lset_socket_boolean
    mov edi, 4
    mov esi, 1
    mov edx, 2
    call .Lget_socket_boolean
    mov edi, 4
    mov esi, 6
    mov edx, 1
    call .Lset_socket_boolean
    mov edi, 4
    mov esi, 6
    mov edx, 1
    call .Lget_socket_boolean
    mov edi, 4
    mov esi, 1
    mov edx, 9
    call .Lset_socket_boolean
    mov edi, 4
    mov esi, 1
    mov edx, 9
    call .Lget_socket_boolean
    mov edi, 4
    xor esi, esi
    mov edx, 11
    call .Lset_socket_boolean
    mov edi, 4
    xor esi, esi
    mov edx, 11
    call .Lget_socket_boolean
    mov qword ptr [rip + .Lstatus], 2
    mov qword ptr [rip + .Lstatus + 8], 345
    mov edi, 4
    mov esi, 1
    mov edx, 20
    lea r10, [rip + .Lstatus]
    mov r8d, 16
    mov eax, 54
    syscall
    call .Lrequire_zero
    mov qword ptr [rip + .Lstatus], 0
    mov qword ptr [rip + .Lstatus + 8], 0
    mov dword ptr [rip + .Lstatus + 16], 16
    mov edi, 4
    mov esi, 1
    mov edx, 20
    lea r10, [rip + .Lstatus]
    lea r8, [rip + .Lstatus + 16]
    mov eax, 55
    syscall
    call .Lrequire_zero
    cmp qword ptr [rip + .Lstatus], 2
    jne .Lfailure
    cmp qword ptr [rip + .Lstatus + 8], 345
    jne .Lfailure
    mov edi, 4
    mov esi, 1
    mov edx, 2
    xor r10d, r10d
    mov r8d, 4
    mov eax, 54
    syscall
    call .Lrequire_efault
    mov edi, 4
    mov esi, 1
    mov edx, 2
    lea r10, [rip + .Lstatus]
    mov r8d, 3
    mov eax, 54
    syscall
    call .Lrequire_einval
    mov edi, 4
    mov esi, 1
    mov edx, 99
    lea r10, [rip + .Lstatus]
    mov r8d, 4
    mov eax, 54
    syscall
    cmp rax, -92
    jne .Lfailure
    mov dword ptr [rip + .Lstatus], 0
    mov dword ptr [rip + .Lstatus + 16], 4
    mov edi, 4
    mov esi, 1
    mov edx, 3
    lea r10, [rip + .Lstatus]
    lea r8, [rip + .Lstatus + 16]
    mov eax, 55
    syscall
    call .Lrequire_zero
    cmp dword ptr [rip + .Lstatus], 1
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 16], 4
    jne .Lfailure
    mov dword ptr [rip + .Lstatus], -1
    mov dword ptr [rip + .Lstatus + 16], 4
    mov edi, 4
    mov esi, 1
    mov edx, 30
    lea r10, [rip + .Lstatus]
    lea r8, [rip + .Lstatus + 16]
    mov eax, 55
    syscall
    call .Lrequire_zero
    cmp dword ptr [rip + .Lstatus], 0
    jne .Lfailure
    mov dword ptr [rip + .Lstatus], 0
    mov dword ptr [rip + .Lstatus + 16], 4
    mov edi, 4
    mov esi, 1
    mov edx, 38
    lea r10, [rip + .Lstatus]
    lea r8, [rip + .Lstatus + 16]
    mov eax, 55
    syscall
    call .Lrequire_zero
    cmp dword ptr [rip + .Lstatus], 6
    jne .Lfailure
    mov dword ptr [rip + .Lstatus], 0
    mov dword ptr [rip + .Lstatus + 16], 4
    mov edi, 4
    mov esi, 1
    mov edx, 39
    lea r10, [rip + .Lstatus]
    lea r8, [rip + .Lstatus + 16]
    mov eax, 55
    syscall
    call .Lrequire_zero
    cmp dword ptr [rip + .Lstatus], 2
    jne .Lfailure
    mov edi, 2
    mov esi, 2
    xor edx, edx
    mov eax, 41
    syscall
    test rax, rax
    js .Lfailure
    mov dword ptr [rip + .Lstatus + 20], eax
    mov dword ptr [rip + .Lstatus], 0
    mov dword ptr [rip + .Lstatus + 16], 4
    mov edi, eax
    mov esi, 1
    mov edx, 3
    lea r10, [rip + .Lstatus]
    lea r8, [rip + .Lstatus + 16]
    mov eax, 55
    syscall
    call .Lrequire_zero
    cmp dword ptr [rip + .Lstatus], 2
    jne .Lfailure
    mov edi, dword ptr [rip + .Lstatus + 20]
    call .Lclose_descriptor
    mov edi, 4
    mov esi, 1
    mov edx, 99
    lea r10, [rip + .Lstatus]
    lea r8, [rip + .Lstatus + 16]
    mov eax, 55
    syscall
    cmp rax, -92
    jne .Lfailure
    mov qword ptr [rip + .Lstatus], 2
    mov qword ptr [rip + .Lstatus + 8], 0
    mov edi, 4
    lea rsi, [rip + .Lstatus]
    mov edx, 16
    mov eax, 49
    syscall
    call .Lrequire_zero
    mov edi, 4
    mov esi, 1
    mov edx, 16
    mov eax, 49
    syscall
    call .Lrequire_efault
    mov edi, 4
    mov rsi, -1
    mov eax, 50
    syscall
    call .Lrequire_einval
    mov edi, 4
    mov esi, 4
    mov eax, 50
    syscall
    call .Lrequire_zero
    mov dword ptr [rip + .Lstatus + 16], 16
    mov edi, 4
    lea rsi, [rip + .Lstatus]
    lea rdx, [rip + .Lstatus + 16]
    mov eax, 51
    syscall
    call .Lrequire_zero
    cmp word ptr [rip + .Lstatus], 2
    jne .Lfailure
    cmp word ptr [rip + .Lstatus + 2], 0
    je .Lfailure
    mov dword ptr [rip + .Lstatus + 16], 16
    mov edi, 4
    mov esi, 1
    lea rdx, [rip + .Lstatus + 16]
    mov eax, 51
    syscall
    call .Lrequire_efault
    mov edi, 4
    xor esi, esi
    xor edx, edx
    mov eax, 43
    syscall
    cmp rax, -11
    jne .Lfailure
    mov edi, 4
    xor esi, esi
    xor edx, edx
    mov r10d, 1
    mov eax, 288
    syscall
    call .Lrequire_einval
    mov edi, 4
    mov esi, 2
    mov eax, 48
    syscall
    cmp rax, -107
    jne .Lfailure
    mov edi, 4
    mov esi, 3
    mov eax, 48
    syscall
    call .Lrequire_einval
    mov edi, 4
    call .Lclose_descriptor

    mov rdi, r15
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
    mov edx, 512
    mov eax, 217
    syscall
    cmp rax, 288
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 67], 0x69757063
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 251], 0x6c646d63
    jne .Lfailure
    cmp byte ptr [rip + .Lstatus + 283], '1'
    jne .Lfailure
    mov rdi, r9
    call .Lclose_descriptor

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
    mov edx, 512
    mov eax, 217
    syscall
    cmp rax, 264
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
    cmp dword ptr [rip + .Lstatus + 227], 0x74617473
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 251], 0x74617473
    jne .Lfailure
    cmp byte ptr [rip + .Lstatus + 255], 'm'
    jne .Lfailure
    mov rdi, r9
    call .Lclose_descriptor

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
    call .Lclose_descriptor

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
    call .Lclose_descriptor

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
    call .Lclose_descriptor

    mov rdi, r15
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
    xor edi, edi
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    mov eax, 101
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
    call .Lrequire_zero
    mov eax, 104
    syscall
    call .Lrequire_zero
    mov rdi, r15
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
    mov rdi, r15
    lea rsi, [rip + .Lstatus]
    mov edx, 45
    mov eax, 1
    syscall
    cmp rax, 45
    jne .Lfailure
    mov rdi, r9
    call .Lclose_descriptor

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
    mov rdi, r15
    lea rsi, [rip + .Lstatus]
    mov edx, 51
    mov eax, 1
    syscall
    cmp rax, 51
    jne .Lfailure
    mov rdi, r9
    call .Lclose_descriptor

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
    call .Lclose_descriptor

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
    call .Lclose_descriptor

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
    call .Lclose_descriptor

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
    call .Lclose_descriptor

    mov edi, -100
    lea rsi, [rip + .Ltmp_file_path]
    xor edx, edx
    mov eax, 263
    syscall
    call .Lrequire_zero

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
    call .Lclose_descriptor

    mov rdi, r15
    lea rsi, [rip + .Ltmp_marker]
    mov edx, 12
    mov eax, 1
    syscall
    cmp rax, 12
    jne .Lfailure

    lea rdi, [rip + .Lmemfd_name]
    mov esi, 1
    mov eax, 319
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    lea rsi, [rip + .Lmemfd_payload]
    mov edx, 5
    mov eax, 1
    syscall
    cmp rax, 5
    jne .Lfailure
    mov rdi, r9
    xor esi, esi
    xor edx, edx
    mov eax, 8
    syscall
    call .Lrequire_zero
    mov rdi, r9
    lea rsi, [rip + .Lstatus]
    mov edx, 5
    xor eax, eax
    syscall
    cmp rax, 5
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus], 0x666d656d
    jne .Lfailure
    cmp byte ptr [rip + .Lstatus + 4], 'd'
    jne .Lfailure
    mov rdi, r9
    xor esi, esi
    mov edx, 5
    mov r10d, 7
    mov eax, 277
    syscall
    call .Lrequire_zero
    mov rdi, r9
    xor esi, esi
    xor edx, edx
    xor r10d, r10d
    mov eax, 277
    syscall
    call .Lrequire_zero
    mov rdi, r9
    xor esi, esi
    mov edx, 5
    mov r10d, 1
    mov eax, 277
    syscall
    call .Lrequire_zero
    mov rdi, r9
    xor esi, esi
    mov edx, 5
    mov r10d, 2
    mov eax, 277
    syscall
    call .Lrequire_zero
    mov rdi, r9
    xor esi, esi
    mov edx, 5
    mov r10d, 4
    mov eax, 277
    syscall
    call .Lrequire_zero
    mov rdi, r9
    xor esi, esi
    mov edx, 5
    mov r10, 0x100000007
    mov eax, 277
    syscall
    call .Lrequire_zero
    mov rdi, 0x100000000
    or rdi, r9
    xor esi, esi
    mov edx, 5
    xor r10d, r10d
    mov eax, 277
    syscall
    call .Lrequire_zero
    mov rdi, r9
    xor esi, esi
    mov edx, 5
    mov r10d, 8
    mov eax, 277
    syscall
    call .Lrequire_einval
    mov rdi, r9
    mov rsi, -1
    mov edx, 5
    xor r10d, r10d
    mov eax, 277
    syscall
    call .Lrequire_einval
    mov rdi, r9
    xor esi, esi
    mov rdx, -1
    xor r10d, r10d
    mov eax, 277
    syscall
    call .Lrequire_einval
    mov rdi, r9
    mov rsi, 0x7fffffffffffffff
    mov edx, 1
    xor r10d, r10d
    mov eax, 277
    syscall
    call .Lrequire_einval
    mov rdi, -1
    xor esi, esi
    mov edx, 5
    xor r10d, r10d
    mov eax, 277
    syscall
    cmp rax, -9
    jne .Lfailure
    mov rdi, r9
    mov eax, 74
    syscall
    call .Lrequire_zero
    mov rdi, r9
    mov eax, 75
    syscall
    call .Lrequire_zero
    mov rdi, 0x100000000
    or rdi, r9
    mov eax, 74
    syscall
    call .Lrequire_zero
    mov rdi, -1
    mov eax, 74
    syscall
    cmp rax, -9
    jne .Lfailure
    mov rdi, r15
    mov eax, 74
    syscall
    call .Lrequire_einval
    lea rdi, [rip + .Lpipe_pair]
    mov eax, 22
    syscall
    call .Lrequire_zero
    mov edi, dword ptr [rip + .Lpipe_pair]
    mov eax, 74
    syscall
    call .Lrequire_einval
    mov edi, dword ptr [rip + .Lpipe_pair + 4]
    mov eax, 75
    syscall
    call .Lrequire_einval
    mov edi, dword ptr [rip + .Lpipe_pair]
    call .Lclose_descriptor
    mov edi, dword ptr [rip + .Lpipe_pair + 4]
    call .Lclose_descriptor
    mov edi, 1
    mov esi, 1
    xor edx, edx
    mov r10, r9
    mov r8, r9
    mov eax, 312
    syscall
    call .Lrequire_zero
    mov edi, 1
    mov esi, 1
    mov edx, 1
    xor r10d, r10d
    xor r8d, r8d
    mov eax, 312
    syscall
    call .Lrequire_zero
    mov rdi, r9
    lea rsi, [rip + .Lcachestat_range]
    lea rdx, [rip + .Lcachestat_result]
    xor r10d, r10d
    mov eax, 451
    syscall
    call .Lrequire_zero
    cmp qword ptr [rip + .Lcachestat_result], 1
    jne .Lfailure
    mov rdi, r9
    call .Lclose_descriptor

    lea rdi, [rip + .Lrseq_area]
    mov esi, 32
    xor edx, edx
    mov r10d, 0x53053053
    mov eax, 334
    syscall
    call .Lrequire_zero
    cmp dword ptr [rip + .Lrseq_area], 0
    jne .Lfailure
    cmp dword ptr [rip + .Lrseq_area + 4], 0
    jne .Lfailure
    lea rdi, [rip + .Lrseq_area]
    mov esi, 32
    xor edx, edx
    mov r10d, 0x53053053
    mov eax, 334
    syscall
    cmp rax, -16
    jne .Lfailure
    lea rdi, [rip + .Lrseq_area]
    mov esi, 32
    mov edx, 1
    mov r10d, 0x53053053
    mov eax, 334
    syscall
    call .Lrequire_zero
    cmp dword ptr [rip + .Lrseq_area], 0
    jne .Lfailure
    cmp dword ptr [rip + .Lrseq_area + 4], -1
    jne .Lfailure

    mov edi, 1
    xor esi, esi
    mov eax, 252
    syscall
    call .Lrequire_zero
    mov edi, 1
    xor esi, esi
    mov edx, 0x4004
    mov eax, 251
    syscall
    call .Lrequire_zero
    mov edi, 1
    mov esi, 1
    mov eax, 252
    syscall
    cmp rax, 0x4004
    jne .Lfailure
    mov edi, 2
    xor esi, esi
    mov eax, 252
    syscall
    cmp rax, 0x4004
    jne .Lfailure
    mov edi, 3
    xor esi, esi
    mov eax, 252
    syscall
    cmp rax, 0x4004
    jne .Lfailure
    mov edi, 1
    xor esi, esi
    mov edx, 1
    mov eax, 251
    syscall
    call .Lrequire_einval
    mov edi, 1
    xor esi, esi
    mov edx, 0x4008
    mov eax, 251
    syscall
    call .Lrequire_einval
    mov edi, 1
    xor esi, esi
    mov edx, 0x8000
    mov eax, 251
    syscall
    call .Lrequire_einval
    xor edi, edi
    xor esi, esi
    mov eax, 252
    syscall
    call .Lrequire_einval
    mov edi, 1
    mov esi, 9999
    mov eax, 252
    syscall
    cmp rax, -3
    jne .Lfailure
    mov rdi, 0x100000001
    mov rsi, 0x100000000
    mov rdx, 0x100004003
    mov eax, 251
    syscall
    call .Lrequire_zero
    mov edi, 1
    xor esi, esi
    mov eax, 252
    syscall
    cmp rax, 0x4003
    jne .Lfailure
    mov edi, 1
    xor esi, esi
    xor edx, edx
    mov eax, 251
    syscall
    call .Lrequire_zero

    xor edi, edi
    xor esi, esi
    mov eax, 140
    syscall
    cmp rax, 20
    jne .Lfailure
    xor edi, edi
    xor esi, esi
    mov edx, 5
    mov eax, 141
    syscall
    call .Lrequire_zero
    xor edi, edi
    mov esi, 1
    mov eax, 140
    syscall
    cmp rax, 15
    jne .Lfailure
    xor edi, edi
    xor esi, esi
    mov edx, -25
    mov eax, 141
    syscall
    call .Lrequire_zero
    mov edi, 1
    xor esi, esi
    mov eax, 140
    syscall
    cmp rax, 40
    jne .Lfailure
    mov edi, 2
    xor esi, esi
    mov eax, 140
    syscall
    cmp rax, 40
    jne .Lfailure
    xor edi, edi
    xor esi, esi
    mov edx, 30
    mov eax, 141
    syscall
    call .Lrequire_zero
    mov rdi, 0x100000000
    mov rsi, 0x100000000
    mov eax, 140
    syscall
    call .Lrequire_one
    mov edi, 3
    xor esi, esi
    mov eax, 140
    syscall
    call .Lrequire_einval
    xor edi, edi
    mov esi, 9999
    mov eax, 140
    syscall
    cmp rax, -3
    jne .Lfailure
    xor edi, edi
    xor esi, esi
    xor edx, edx
    mov eax, 141
    syscall
    call .Lrequire_zero

    mov dword ptr [rip + .Lstatus], 0
    xor edi, edi
    lea rsi, [rip + .Lstatus]
    mov eax, 142
    syscall
    call .Lrequire_zero
    mov edi, 2
    lea rsi, [rip + .Lstatus]
    mov eax, 142
    syscall
    cmp rax, -3
    jne .Lfailure
    xor edi, edi
    mov esi, 1
    mov eax, 142
    syscall
    call .Lrequire_efault
    mov dword ptr [rip + .Lstatus], 1
    xor edi, edi
    lea rsi, [rip + .Lstatus]
    mov eax, 142
    syscall
    call .Lrequire_einval
    mov dword ptr [rip + .Lstatus], 0
    xor edi, edi
    xor esi, esi
    lea rdx, [rip + .Lstatus]
    mov eax, 144
    syscall
    call .Lrequire_zero
    xor edi, edi
    mov esi, 1
    lea rdx, [rip + .Lstatus]
    mov eax, 144
    syscall
    cmp rax, -1
    jne .Lfailure
    xor edi, edi
    mov esi, 4
    lea rdx, [rip + .Lstatus]
    mov eax, 144
    syscall
    call .Lrequire_einval
    mov edi, 2
    xor esi, esi
    lea rdx, [rip + .Lstatus]
    mov eax, 144
    syscall
    cmp rax, -3
    jne .Lfailure
    xor edi, edi
    xor esi, esi
    mov edx, 1
    mov eax, 144
    syscall
    call .Lrequire_efault

    xor edi, edi
    lea rsi, [rip + .Lsched_attr]
    mov edx, 56
    xor r10d, r10d
    mov eax, 315
    syscall
    call .Lrequire_zero
    cmp dword ptr [rip + .Lsched_attr], 56
    jne .Lfailure
    cmp dword ptr [rip + .Lsched_attr + 4], 0
    jne .Lfailure
    cmp dword ptr [rip + .Lsched_attr + 52], 1024
    jne .Lfailure
    mov edi, 2
    lea rsi, [rip + .Lsched_attr]
    mov edx, 56
    xor r10d, r10d
    mov eax, 315
    syscall
    cmp rax, -3
    jne .Lfailure
    mov rdi, -1
    lea rsi, [rip + .Lsched_attr]
    mov edx, 56
    xor r10d, r10d
    mov eax, 315
    syscall
    call .Lrequire_einval
    xor edi, edi
    lea rsi, [rip + .Lsched_attr]
    mov edx, 55
    xor r10d, r10d
    mov eax, 315
    syscall
    call .Lrequire_einval
    xor edi, edi
    lea rsi, [rip + .Lsched_attr]
    mov edx, 56
    mov r10d, 1
    mov eax, 315
    syscall
    call .Lrequire_einval
    xor edi, edi
    mov esi, 1
    mov edx, 56
    xor r10d, r10d
    mov eax, 315
    syscall
    call .Lrequire_efault
    xor edi, edi
    lea rsi, [rip + .Lsched_attr]
    xor edx, edx
    mov eax, 314
    syscall
    call .Lrequire_zero
    mov edi, 2
    lea rsi, [rip + .Lsched_attr]
    xor edx, edx
    mov eax, 314
    syscall
    cmp rax, -3
    jne .Lfailure
    mov rdi, -1
    lea rsi, [rip + .Lsched_attr]
    xor edx, edx
    mov eax, 314
    syscall
    call .Lrequire_einval
    xor edi, edi
    lea rsi, [rip + .Lsched_attr]
    mov edx, 1
    mov eax, 314
    syscall
    call .Lrequire_einval
    xor edi, edi
    mov esi, 1
    xor edx, edx
    mov eax, 314
    syscall
    call .Lrequire_efault
    mov dword ptr [rip + .Lsched_attr], 47
    xor edi, edi
    lea rsi, [rip + .Lsched_attr]
    xor edx, edx
    mov eax, 314
    syscall
    cmp rax, -7
    jne .Lfailure
    mov dword ptr [rip + .Lsched_attr], 56
    mov dword ptr [rip + .Lsched_attr + 4], 4
    xor edi, edi
    lea rsi, [rip + .Lsched_attr]
    xor edx, edx
    mov eax, 314
    syscall
    call .Lrequire_einval
    mov dword ptr [rip + .Lsched_attr + 4], 1
    xor edi, edi
    lea rsi, [rip + .Lsched_attr]
    xor edx, edx
    mov eax, 314
    syscall
    cmp rax, -1
    jne .Lfailure
    mov dword ptr [rip + .Lsched_attr + 4], 0
    mov dword ptr [rip + .Lsched_attr + 16], 1
    xor edi, edi
    lea rsi, [rip + .Lsched_attr]
    xor edx, edx
    mov eax, 314
    syscall
    call .Lrequire_einval
    mov dword ptr [rip + .Lsched_attr + 16], 0

    mov dword ptr [rip + .Ltimer_event + 12], 1
    mov edi, 1
    lea rsi, [rip + .Ltimer_event]
    lea rdx, [rip + .Ltimer_id]
    mov eax, 222
    syscall
    call .Lrequire_zero
    cmp dword ptr [rip + .Ltimer_id], 1
    jne .Lfailure
    mov qword ptr [rip + .Ltimer_new + 16], 60
    mov edi, 1
    xor esi, esi
    lea rdx, [rip + .Ltimer_new]
    xor r10d, r10d
    mov eax, 223
    syscall
    call .Lrequire_zero
    mov edi, 1
    lea rsi, [rip + .Ltimer_current]
    mov eax, 224
    syscall
    call .Lrequire_zero
    cmp qword ptr [rip + .Ltimer_current + 16], 0
    je .Lfailure
    mov edi, 1
    mov eax, 225
    syscall
    call .Lrequire_zero
    mov edi, 1
    mov eax, 226
    syscall
    call .Lrequire_zero
    mov edi, 1
    lea rsi, [rip + .Ltimer_current]
    mov eax, 224
    syscall
    call .Lrequire_einval

    lea rax, [rip + .Lprocess_vm_output]
    mov qword ptr [rip + .Lprocess_vm_local], rax
    mov qword ptr [rip + .Lprocess_vm_local + 8], 4
    add rax, 4
    mov qword ptr [rip + .Lprocess_vm_local + 16], rax
    mov qword ptr [rip + .Lprocess_vm_local + 24], 6
    lea rax, [rip + .Lprocess_vm_source]
    mov qword ptr [rip + .Lprocess_vm_remote], rax
    mov qword ptr [rip + .Lprocess_vm_remote + 8], 10
    mov edi, 1
    lea rsi, [rip + .Lprocess_vm_local]
    mov edx, 2
    lea r10, [rip + .Lprocess_vm_remote]
    mov r8d, 1
    xor r9d, r9d
    mov eax, 310
    syscall
    cmp rax, 10
    jne .Lfailure
    mov rax, qword ptr [rip + .Lprocess_vm_source]
    cmp qword ptr [rip + .Lprocess_vm_output], rax
    jne .Lfailure
    mov ax, word ptr [rip + .Lprocess_vm_source + 8]
    cmp word ptr [rip + .Lprocess_vm_output + 8], ax
    jne .Lfailure
    lea rax, [rip + .Lprocess_vm_output]
    mov qword ptr [rip + .Lprocess_vm_local], rax
    mov qword ptr [rip + .Lprocess_vm_local + 8], 10
    lea rax, [rip + .Lprocess_vm_target]
    mov qword ptr [rip + .Lprocess_vm_remote], rax
    mov edi, 1
    lea rsi, [rip + .Lprocess_vm_local]
    mov edx, 1
    lea r10, [rip + .Lprocess_vm_remote]
    mov r8d, 1
    xor r9d, r9d
    mov eax, 311
    syscall
    cmp rax, 10
    jne .Lfailure
    mov rax, qword ptr [rip + .Lprocess_vm_source]
    cmp qword ptr [rip + .Lprocess_vm_target], rax
    jne .Lfailure
    mov ax, word ptr [rip + .Lprocess_vm_source + 8]
    cmp word ptr [rip + .Lprocess_vm_target + 8], ax
    jne .Lfailure

    xor edi, edi
    xor esi, esi
    xor edx, edx
    mov eax, 238
    syscall
    call .Lrequire_zero
    lea rdi, [rip + .Lnuma_mode]
    lea rsi, [rip + .Lnuma_mask]
    mov edx, 64
    xor r10d, r10d
    mov r8d, 4
    mov eax, 239
    syscall
    call .Lrequire_zero
    cmp dword ptr [rip + .Lnuma_mode], 0
    jne .Lfailure
    cmp qword ptr [rip + .Lnuma_mask], 1
    jne .Lfailure
    lea rdi, [rip + .Lstatus]
    and rdi, -4096
    mov esi, 4096
    xor edx, edx
    xor r10d, r10d
    xor r8d, r8d
    xor r9d, r9d
    mov eax, 237
    syscall
    call .Lrequire_zero
    xor edi, edi
    mov esi, 64
    lea rdx, [rip + .Lnuma_mask]
    lea r10, [rip + .Lnuma_mask]
    mov eax, 256
    syscall
    call .Lrequire_zero
    lea rax, [rip + .Lstatus]
    and rax, -4096
    mov qword ptr [rip + .Lnuma_page], rax
    xor edi, edi
    mov esi, 1
    lea rdx, [rip + .Lnuma_page]
    lea r10, [rip + .Lnuma_node]
    lea r8, [rip + .Lnuma_status]
    xor r9d, r9d
    mov eax, 279
    syscall
    call .Lrequire_zero
    cmp dword ptr [rip + .Lnuma_status], 0
    jne .Lfailure
    lea rdi, [rip + .Lstatus]
    and rdi, -4096
    mov esi, 4096
    xor edx, edx
    xor r10d, r10d
    mov eax, 450
    syscall
    call .Lrequire_zero

    lea rax, [rip + .Lsignal_handler]
    mov qword ptr [rip + .Lsignal_action], rax
    mov qword ptr [rip + .Lsignal_action + 8], 0x4000000
    lea rax, [rip + .Lsignal_restorer]
    mov qword ptr [rip + .Lsignal_action + 16], rax
    mov qword ptr [rip + .Lsignal_action + 24], 0
    mov edi, 10
    lea rsi, [rip + .Lsignal_action]
    lea rdx, [rip + .Lsignal_old_usr1]
    mov r10d, 8
    mov eax, 13
    syscall
    call .Lrequire_zero

    mov qword ptr [rip + .Lsignal_set], 0x200
    xor edi, edi
    lea rsi, [rip + .Lsignal_set]
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    call .Lrequire_zero

    mov dword ptr [rip + .Lqueue_info], 10
    mov dword ptr [rip + .Lqueue_info + 8], -1
    mov rax, 0x1122334455667788
    mov qword ptr [rip + .Lqueue_info + 24], rax
    mov edi, 1
    mov esi, 10
    lea rdx, [rip + .Lqueue_info]
    mov eax, 129
    syscall
    call .Lrequire_zero

    lea rdi, [rip + .Lstatus]
    mov esi, 8
    mov eax, 127
    syscall
    call .Lrequire_zero
    cmp qword ptr [rip + .Lstatus], 0x200
    jne .Lfailure

    lea rdi, [rip + .Lsignal_set]
    lea rsi, [rip + .Lstatus]
    xor edx, edx
    mov r10d, 8
    mov eax, 128
    syscall
    cmp rax, 10
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus], 10
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 8], -1
    jne .Lfailure
    mov rax, 0x1122334455667788
    cmp qword ptr [rip + .Lstatus + 24], rax
    jne .Lfailure

    mov edi, 1
    mov esi, 1
    mov edx, 10
    lea r10, [rip + .Lqueue_info]
    mov eax, 297
    syscall
    call .Lrequire_zero

    lea rdi, [rip + .Lsignal_set]
    lea rsi, [rip + .Lstatus]
    xor edx, edx
    mov r10d, 8
    mov eax, 128
    syscall
    cmp rax, 10
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 8], -1
    jne .Lfailure
    mov rax, 0x1122334455667788
    cmp qword ptr [rip + .Lstatus + 24], rax
    jne .Lfailure

    lea rdi, [rip + .Lstatus]
    mov esi, 8
    mov eax, 127
    syscall
    call .Lrequire_zero
    cmp qword ptr [rip + .Lstatus], 0
    jne .Lfailure

    mov rdi, -1
    lea rsi, [rip + .Lsignal_set]
    mov edx, 8
    mov r10d, 0x80000
    mov eax, 289
    syscall
    test rax, rax
    js .Lfailure
    mov r12, rax
    mov edi, 1
    mov esi, 10
    mov eax, 62
    syscall
    call .Lrequire_zero
    mov rdi, r12
    lea rsi, [rip + .Lsignalfd_info]
    mov edx, 128
    xor eax, eax
    syscall
    cmp rax, 128
    jne .Lfailure
    cmp dword ptr [rip + .Lsignalfd_info], 10
    jne .Lfailure
    mov rdi, r12
    call .Lclose_descriptor

    mov edi, 1
    lea rsi, [rip + .Lsignal_set]
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    call .Lrequire_zero

    mov qword ptr [rip + .Lsignal_action], 1
    mov qword ptr [rip + .Lsignal_action + 8], 0
    mov qword ptr [rip + .Lsignal_action + 16], 0
    mov qword ptr [rip + .Lsignal_action + 24], 0
    mov edi, 12
    lea rsi, [rip + .Lsignal_action]
    lea rdx, [rip + .Lsignal_old_usr2]
    mov r10d, 8
    mov eax, 13
    syscall
    call .Lrequire_zero
    mov edi, 1
    mov esi, 12
    mov eax, 62
    syscall
    call .Lrequire_zero

    mov edi, 10
    lea rsi, [rip + .Lsignal_old_usr1]
    xor edx, edx
    mov r10d, 8
    mov eax, 13
    syscall
    call .Lrequire_zero
    mov edi, 12
    lea rsi, [rip + .Lsignal_old_usr2]
    mov eax, 13
    syscall
    call .Lrequire_zero

    mov rdi, r15
    lea rsi, [rip + .Lsignal_marker]
    mov edx, 13
    mov eax, 1
    syscall
    cmp rax, 13
    jne .Lfailure

    lea rdi, [rip + .Lfutex_word]
    mov esi, 128
    xor edx, edx
    xor r10d, r10d
    mov eax, 202
    syscall
    cmp rax, -11
    jne .Lfailure
    cmp dword ptr [rip + .Lfutex_word], 1
    jne .Lfailure

    lea rdi, [rip + .Lfutex_word]
    mov esi, 129
    mov edx, 1
    xor r10d, r10d
    mov eax, 202
    syscall
    call .Lrequire_zero

    lea rdi, [rip + .Lfutex_word]
    mov esi, -1
    mov edx, 1
    mov r10d, 2
    mov eax, 454
    syscall
    call .Lrequire_zero
    lea rdi, [rip + .Lfutex_word]
    xor esi, esi
    mov edx, -1
    mov r10d, 2
    xor r8d, r8d
    mov r9d, 1
    mov eax, 455
    syscall
    cmp rax, -11
    jne .Lfailure
    lea rax, [rip + .Lfutex_word]
    mov qword ptr [rip + .Lfutex_requeue + 8], rax
    lea rax, [rip + .Lfutex_target]
    mov qword ptr [rip + .Lfutex_requeue + 32], rax
    lea rdi, [rip + .Lfutex_requeue]
    xor esi, esi
    mov edx, 1
    mov r10d, 1
    mov eax, 456
    syscall
    call .Lrequire_zero
    lea rax, [rip + .Lfutex_word]
    mov qword ptr [rip + .Lfutex_waitv + 8], rax
    lea rdi, [rip + .Lfutex_waitv]
    mov esi, 1
    xor edx, edx
    xor r10d, r10d
    mov r8d, 1
    mov eax, 449
    syscall
    cmp rax, -11
    jne .Lfailure

    mov rdi, r15
    lea rsi, [rip + .Lfutex_marker]
    mov edx, 12
    mov eax, 1
    syscall
    cmp rax, 12
    jne .Lfailure

    xor edi, edi
    xor esi, esi
    mov edx, 5
    mov eax, 141
    syscall
    call .Lrequire_zero
    mov edi, 1
    xor esi, esi
    mov edx, 0x4002
    mov eax, 251
    syscall
    call .Lrequire_zero

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
    mov edx, 512
    mov eax, 217
    syscall
    cmp rax, 312
    jne .Lfailure
    cmp byte ptr [rip + .Lstatus + 307], '2'
    jne .Lfailure
    mov rdi, r9
    call .Lclose_descriptor

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
    mov edx, 512
    mov eax, 217
    syscall
    cmp rax, 136
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 67], 0x74617473
    jne .Lfailure
    mov rdi, r9
    call .Lclose_descriptor

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
    call .Lclose_descriptor

    mov rdi, r12
    xor esi, esi
    mov edx, 0x80000000
    xor r10d, r10d
    mov eax, 61
    syscall
    call .Lrequire_einval

    mov rdi, r12
    mov esi, 1
    xor edx, edx
    xor r10d, r10d
    mov eax, 61
    syscall
    call .Lrequire_efault

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

    xor edi, edi
    xor esi, esi
    xor edx, edx
    mov eax, 141
    syscall
    call .Lrequire_zero
    mov edi, 1
    xor esi, esi
    xor edx, edx
    mov eax, 251
    syscall
    call .Lrequire_zero

    mov edi, -100
    lea rsi, [rip + .Lpid2_status_path]
    xor edx, edx
    mov eax, 257
    syscall
    cmp rax, -2
    jne .Lfailure

    mov rdi, r15
    lea rsi, [rip + .Lprocfs_marker]
    mov edx, 13
    mov eax, 1
    syscall
    cmp rax, 13
    jne .Lfailure
    jmp .Lfork_complete

.Lfork_child:
    mov edi, 1
    xor esi, esi
    mov eax, 252
    syscall
    cmp rax, 0x4002
    jne .Lfailure
    xor edi, edi
    xor esi, esi
    mov eax, 140
    syscall
    cmp rax, 15
    jne .Lfailure
    xor edi, edi
    xor esi, esi
    mov edx, 10
    mov eax, 141
    syscall
    call .Lrequire_zero
    mov eax, 39
    syscall
    cmp rax, 2
    jne .Lfailure
    mov eax, 110
    syscall
    call .Lrequire_one
    mov eax, 102
    syscall
    call .Lrequire_zero
    mov eax, 104
    syscall
    call .Lrequire_zero
    mov edi, 42
    mov eax, 60
    syscall
    ud2

.Lfork_complete:

    # Verify gettimeofday conversion, timezone compatibility, and bounds.
    lea rdi, [rip + .Lstatus]
    lea rsi, [rip + .Lstatus + 16]
    mov eax, 96
    syscall
    call .Lrequire_zero
    cmp qword ptr [rip + .Lstatus], 1577836800
    jb .Lfailure
    cmp qword ptr [rip + .Lstatus + 8], 999999
    ja .Lfailure
    cmp qword ptr [rip + .Lstatus + 16], 0
    jne .Lfailure
    mov edi, 1
    xor esi, esi
    mov eax, 96
    syscall
    call .Lrequire_efault
    xor edi, edi
    xor esi, esi
    mov eax, 96
    syscall
    call .Lrequire_zero

    # Verify legacy time returns and optionally stores the realtime second.
    lea rdi, [rip + .Lstatus]
    mov eax, 201
    syscall
    cmp rax, 1577836800
    jb .Lfailure
    cmp qword ptr [rip + .Lstatus], rax
    jne .Lfailure
    mov edi, 1
    mov eax, 201
    syscall
    call .Lrequire_efault

    # Verify close_range marking, closure, and Linux error boundaries.
    mov edi, -100
    lea rsi, [rip + .Lnull_path]
    mov edx, 2
    mov eax, 257
    syscall
    cmp rax, 3
    jb .Lfailure
    mov r9, rax
    mov rdi, rax
    mov rsi, rax
    mov edx, 4
    mov eax, 436
    syscall
    call .Lrequire_zero
    mov rdi, r9
    mov esi, 1
    xor edx, edx
    mov eax, 72
    syscall
    call .Lrequire_one
    mov rdi, r9
    mov esi, -1
    xor edx, edx
    mov eax, 436
    syscall
    call .Lrequire_zero
    mov rdi, r9
    mov esi, 1
    xor edx, edx
    mov eax, 72
    syscall
    cmp rax, -9
    jne .Lfailure
    mov edi, 4
    mov esi, 3
    xor edx, edx
    mov eax, 436
    syscall
    call .Lrequire_einval
    mov edi, 3
    mov esi, 3
    mov edx, 1
    mov eax, 436
    syscall
    call .Lrequire_einval
    mov edi, -100
    lea rsi, [rip + .Lnull_path]
    mov edx, 2
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, rax
    mov rsi, rax
    mov edx, 2
    mov eax, 436
    syscall
    call .Lrequire_zero
    mov rdi, r9
    mov esi, 1
    xor edx, edx
    mov eax, 72
    syscall
    cmp rax, -9
    jne .Lfailure
    mov edi, -100
    lea rsi, [rip + .Lnull_path]
    mov edx, 2
    mov eax, 257
    syscall
    test rax, rax
    js .Lfailure
    mov r9, rax
    mov rdi, 0x100000000
    or rdi, r9
    mov rsi, 0x100000000
    or rsi, r9
    mov rdx, 0x100000006
    mov eax, 436
    syscall
    call .Lrequire_zero
    mov rdi, r9
    mov esi, 1
    xor edx, edx
    mov eax, 72
    syscall
    call .Lrequire_one
    mov rdi, r9
    call .Lclose_descriptor

    mov qword ptr [rip + .Lsignal_set], 0x4000
    xor edi, edi
    lea rsi, [rip + .Lsignal_set]
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    call .Lrequire_zero

    mov rdi, r15
    lea rsi, [rip + .Lready]
    mov edx, 21
    mov eax, 1
    syscall
    cmp rax, 21
    jne .Lfailure

    lea rdi, [rip + .Lsignal_set]
    lea rsi, [rip + .Lstatus]
    xor edx, edx
    mov r10d, 8
    mov eax, 128
    syscall
    cmp rax, 15
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus], 15
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus + 8], 0x80
    jne .Lfailure

    mov rdi, r15
    lea rsi, [rip + .Lsupervisor_shutdown_marker]
    mov edx, 20
    mov eax, 1
    syscall
    cmp rax, 20
    jne .Lfailure

    mov edi, 1
    lea rsi, [rip + .Lsignal_set]
    xor edx, edx
    mov r10d, 8
    mov eax, 14
    syscall
    call .Lrequire_zero

    mov rdi, r15
    call .Lclose_descriptor

    mov edi, 42
    mov eax, 231
    syscall

.Lset_socket_boolean:
    mov dword ptr [rip + .Lstatus], 1
    lea r10, [rip + .Lstatus]
    mov r8d, 4
    mov eax, 54
    syscall
    jmp .Lrequire_zero

.Lget_socket_boolean:
    mov dword ptr [rip + .Lstatus], 0
    mov dword ptr [rip + .Lstatus + 16], 4
    lea r10, [rip + .Lstatus]
    lea r8, [rip + .Lstatus + 16]
    mov eax, 55
    syscall
    test rax, rax
    jne .Lfailure
    cmp dword ptr [rip + .Lstatus], 1
    jne .Lfailure
    ret

.Lclose_descriptor:
    mov eax, 3
    syscall
    jmp .Lrequire_zero

.Lrequire_zero:
    test rax, rax
    jne .Lfailure
    ret

.Lrequire_one:
    cmp rax, 1
    jne .Lfailure
    ret

.Lrequire_einval:
    cmp rax, -22
    jne .Lfailure
    ret

.Lrequire_efault:
    cmp rax, -14
    jne .Lfailure
    ret

.Lfailure:
    mov edi, 1
    mov eax, 231
    syscall
    ud2

.Lsignal_handler:
    jmp .Lfailure

.Lsignal_restorer:
    mov eax, 15
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
.Lmemfd_name:
    .asciz "ginux-test"
.Lmemfd_payload:
    .ascii "memfd"
.Ltmpfs_name:
    .asciz "tmpfs"
.Lunknown_fs_name:
    .asciz "unknown"
.Lsysfs_name:
    .space 9
.align 8
.Lustat:
    .space 32
.align 8
.Lsocket_pair:
    .space 8
.Lipv4_socket:
    .long 0
.Ldns_socket:
    .long 0
.align 2
.Ldns_address:
    .word 2
    .word 0x3500
    .long 0x0302000a
    .quad 0
.Ldns_query:
    .byte 0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00
    .byte 0x00, 0x00, 0x00, 0x00
    .byte 7
    .ascii "example"
    .byte 3
    .ascii "com"
    .byte 0, 0, 1, 0, 1
.align 2
.Lipv4_peer:
    .word 2
    .byte 0, 53
    .byte 10, 0, 2, 2
    .space 8
.align 4
.Ltcp_listener:
    .long 0
.Ltcp_client:
    .long 0
.Ltcp_client_second:
    .long 0
.Ltcp_accepted:
    .long 0
.Ltcp_address_length:
    .long 16
.Ltcp_peer_length:
    .long 16
.align 2
.Ltcp_bind:
    .word 2
    .byte 0x46, 0xa0
    .space 12
.Ltcp_address:
    .space 16
.Ltcp_peer:
    .space 16
.Ltcp_payload:
    .ascii "tcp!"
.Ltcp_output:
    .space 4
.Lmmsg_payload:
    .ascii "mmsg!"
.Lmmsg_output:
    .space 5
.Lright_payload:
    .byte 'R'
.Lright_output:
    .byte 0
.align 4
.Lright_source:
    .long 0
.align 8
.Lmmsg_send_iov:
    .space 16
.Lmmsg_recv_iov:
    .space 16
.Lmmsg_send:
    .space 64
.Lmmsg_recv:
    .space 64
.Lright_control:
    .space 24
.Lright_output_control:
    .space 56
.align 8
.Lpipe_pair:
    .space 8
.Lvmsplice_payload:
    .ascii "vsplice!"
.Lvmsplice_output:
    .space 8
.align 8
.Lvmsplice_iov:
    .space 16
.align 8
.Lcachestat_range:
    .quad 0
    .quad 0
.Lcachestat_result:
    .space 40
.align 8
.Lsched_attr:
    .space 56
.align 32
.Lrseq_area:
    .space 32
.align 8
.Ltimer_event:
    .space 64
.Ltimer_id:
    .long 0
.align 8
.Ltimer_new:
    .space 32
.Ltimer_current:
    .space 32
.align 8
.Lprocess_vm_source:
    .ascii "process-vm"
.Lprocess_vm_output:
    .space 10
.Lprocess_vm_target:
    .space 10
.align 8
.Lprocess_vm_local:
    .space 32
.Lprocess_vm_remote:
    .space 16
.align 8
.Lnuma_mask:
    .quad 1
.Lnuma_page:
    .quad 0
.Lnuma_mode:
    .long -1
.Lnuma_node:
    .long 0
.Lnuma_status:
    .long -1
.Ldevfs_marker:
    .ascii "DEVFS_READY\n"
.Lprocfs_marker:
    .ascii "PROCFS_READY\n"
.Lsignal_marker:
    .ascii "SIGNAL_READY\n"
.align 4
.Lfutex_word:
    .long 1
.Lfutex_target:
    .long 0
.align 8
.Lfutex_requeue:
    .quad 1
    .quad 0
    .long 2
    .long 0
    .quad 0
    .quad 0
    .long 2
    .long 0
.Lfutex_waitv:
    .quad 0
    .quad 0
    .long 2
    .long 0
.Lfutex_marker:
    .ascii "FUTEX_READY\n"
.Lsupervisor_shutdown_marker:
    .ascii "SUPERVISOR_SHUTDOWN\n"
.align 8
.Lsignal_set:
    .quad 0
.Lsignal_action:
    .space 32
.Lsignal_old_usr1:
    .space 32
.Lsignal_old_usr2:
    .space 32
.Lsignal_old_pipe:
    .space 32
.Lqueue_info:
    .space 128
.Lsignalfd_info:
    .space 128
.Luserns_marker:
    .ascii "USERNS_READY\n"
.Lprivilege_marker:
    .ascii "PRIVILEGE_BOUNDARY_READY\n"
.Lstatus:
    .space 512

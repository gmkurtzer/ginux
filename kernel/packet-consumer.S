.text
.balign 16
.global ginux_packet_consumer_start
.global ginux_packet_consumer_end

ginux_packet_consumer_start:
    mov r12, rbx
    mov r14, rcx
    mov r15, r8
    mov rdi, rcx
    sub rsp, 0x90

    # Request v0: size/version, buffer, capacity, reserved, expected sequence.
    mov qword ptr [rsp], 0x20
    lea rax, [rsp + 0x40]
    mov qword ptr [rsp + 8], rax
    mov qword ptr [rsp + 16], 64
    mov qword ptr [rsp + 24], 0
    movabs rax, 0x5a5a5a5a5a5a5a5a
    mov qword ptr [rsp + 0x40], rax

    # The read-only consumer cannot publish.
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    cmp rax, -3
    jne .consumer_failed
    movabs rax, 0x5a5a5a5a5a5a5a5a
    cmp qword ptr [rsp + 0x40], rax
    jne .consumer_failed

    # A future sequence is rejected without consuming or modifying output.
    lea r13, [r12 + 1]
    mov qword ptr [rsp + 24], r13
    lea rsi, [rsp]
    mov eax, 22
    int 0x80
    cmp rax, -5
    jne .consumer_failed
    movabs rax, 0x5a5a5a5a5a5a5a5a
    cmp qword ptr [rsp + 0x40], rax
    jne .consumer_failed

    # Consume exact sequence one and compare all 42 Ethernet/ARP bytes.
    mov qword ptr [rsp + 24], r12
    lea rsi, [rsp]
    mov eax, 22
    int 0x80
    cmp rax, 42
    jne .consumer_failed
    lea rsi, [rip + .expected_arp]
    lea rdi, [rsp + 0x40]
    mov ecx, 42
    repe cmpsb
    jne .consumer_failed

    # Delivery is destructive only after a successful copy.
    mov rdi, r14
    mov qword ptr [rsp + 24], r12
    lea rsi, [rsp]
    mov eax, 22
    int 0x80
    test r15, r15
    jz .expect_empty
    cmp rax, -5
    jne .consumer_failed
    jmp .replay_checked
.expect_empty:
    cmp rax, -3
    jne .consumer_failed
.replay_checked:

    mov esi, 47
    mov eax, 2
    int 0x80

.consumer_failed:
    int3

.expected_arp:
    .byte 0x52, 0x54, 0x00, 0x12, 0x34, 0x56
    .byte 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02
    .byte 0x08, 0x06
    .byte 0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x02
    .byte 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02
    .byte 10, 0, 2, 2
    .byte 0x52, 0x54, 0x00, 0x12, 0x34, 0x56
    .byte 10, 0, 2, 15

ginux_packet_consumer_end:

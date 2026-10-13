.text
.balign 16
.global ginux_packet_egress_consumer_start
.global ginux_packet_egress_consumer_end

ginux_packet_egress_consumer_start:
    mov r12, rbx
    mov r14, rcx
    sub rsp, 0x90

    # Request v0: size/version, buffer, capacity, reserved, expected sequence.
    mov qword ptr [rsp], 0x20
    lea rax, [rsp + 0x40]
    mov qword ptr [rsp + 8], rax
    mov qword ptr [rsp + 16], 64
    mov qword ptr [rsp + 24], 0
    movabs rax, 0x5a5a5a5a5a5a5a5a
    mov qword ptr [rsp + 0x40], rax

    # The read-only egress consumer cannot publish.
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    cmp rax, -3
    jne .egress_failed
    movabs rax, 0x5a5a5a5a5a5a5a5a
    cmp qword ptr [rsp + 0x40], rax
    jne .egress_failed

    # A future sequence is stale and does not alter output.
    lea rax, [r12 + 1]
    mov qword ptr [rsp + 24], rax
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 22
    int 0x80
    cmp rax, -5
    jne .egress_failed
    movabs rax, 0x5a5a5a5a5a5a5a5a
    cmp qword ptr [rsp + 0x40], rax
    jne .egress_failed

    # Consume and compare the exact protocol-generated echo reply.
    mov qword ptr [rsp + 24], r12
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 22
    int 0x80
    cmp rax, 42
    jne .egress_failed
    lea rsi, [rip + .expected_reply]
    lea rdi, [rsp + 0x40]
    mov ecx, 42
    repe cmpsb
    jne .egress_failed

    # Destructive delivery rejects replay after the successful copy.
    mov rdi, r14
    mov qword ptr [rsp + 24], r12
    lea rsi, [rsp]
    mov eax, 22
    int 0x80
    cmp rax, -3
    jne .egress_failed

    mov esi, 56
    mov eax, 2
    int 0x80

.egress_failed:
    int3

.expected_reply:
    .byte 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02
    .byte 0x52, 0x54, 0x00, 0x12, 0x34, 0x56
    .byte 0x08, 0x00
    .byte 0x45, 0x00, 0x00, 0x1c, 0x12, 0x34, 0x40, 0x00
    .byte 0x40, 0x01, 0x10, 0x9d, 10, 0, 2, 15, 10, 0, 2, 2
    .byte 0x00, 0x00, 0x41, 0x0f, 0xbe, 0xef, 0x00, 0x01

ginux_packet_egress_consumer_end:

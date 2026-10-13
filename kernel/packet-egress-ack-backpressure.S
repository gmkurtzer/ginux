.text
.balign 16
.global ginux_packet_egress_ack_backpressure_producer_start
.global ginux_packet_egress_ack_backpressure_producer_end

ginux_packet_egress_ack_backpressure_producer_start:
    mov r12, rbx
    mov r14, rcx
    sub rsp, 0xa0
    mov qword ptr [rsp + 0x80], r14
    mov qword ptr [rsp + 0x88], r12
    mov qword ptr [rsp], 0x20
    lea rax, [rsp + 0x40]
    mov qword ptr [rsp + 8], rax
    mov qword ptr [rsp + 16], 42
    mov qword ptr [rsp + 24], 0
    lea rsi, [rip + .ack_backpressure_reply]
    lea rdi, [rsp + 0x40]
    mov ecx, 42
    rep movsb
    xor r13d, r13d

    # Fill all four egress slots before the network service can run.
.ack_backpressure_fill:
    mov r15, r12
    add r15, r13
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    mov r12, qword ptr [rsp + 0x88]
    mov r14, qword ptr [rsp + 0x80]
    cmp rax, r15
    jne .ack_backpressure_failed
    inc r13
    cmp r13, 4
    jb .ack_backpressure_fill

    # A claim cannot yet exist, and a fifth publication sees a full FIFO.
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    mov r12, qword ptr [rsp + 0x88]
    mov r14, qword ptr [rsp + 0x80]
    cmp rax, -3
    jne .ack_backpressure_failed

    # Sleep until completed acknowledgement removes the transmitted head.
    mov rdi, r14
    xor esi, esi
    xor edx, edx
    mov eax, 29
    int 0x80
    mov r12, qword ptr [rsp + 0x88]
    mov r14, qword ptr [rsp + 0x80]
    test rax, rax
    jne .ack_backpressure_failed

    # Retry adds the fifth reply at the exact next sequence.
    mov r15, r12
    add r15, 4
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    cmp rax, r15
    jne .ack_backpressure_failed

    mov esi, 62
    mov eax, 2
    int 0x80

.ack_backpressure_failed:
    int3

.ack_backpressure_reply:
    .byte 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02
    .byte 0x52, 0x54, 0x00, 0x12, 0x34, 0x56
    .byte 0x08, 0x00
    .byte 0x45, 0x00, 0x00, 0x1c, 0x12, 0x34, 0x40, 0x00
    .byte 0x40, 0x01, 0x10, 0x9d
    .byte 10, 0, 2, 15
    .byte 10, 0, 2, 2
    .byte 0x00, 0x00, 0x41, 0x0f, 0xbe, 0xef, 0x00, 0x01

ginux_packet_egress_ack_backpressure_producer_end:

.text
.balign 16
.global ginux_packet_egress_batch_producer_start
.global ginux_packet_egress_batch_producer_end

ginux_packet_egress_batch_producer_start:
    mov r12, rbx
    mov r14, rcx
    sub rsp, 0x90
    mov qword ptr [rsp + 0x70], r14
    mov qword ptr [rsp + 0x78], r12

    # A write-only producer cannot use the consumer wait operation.
    mov qword ptr [rsp], 0x20
    mov qword ptr [rsp + 8], 0
    mov qword ptr [rsp + 16], 0
    mov qword ptr [rsp + 24], r12
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 28
    int 0x80
    mov r12, qword ptr [rsp + 0x78]
    mov r14, qword ptr [rsp + 0x70]
    cmp rax, -3
    jne .egress_batch_failed

    # Let the resident network service block on the first exact sequence.
    mov eax, 3
    int 0x80
    mov r12, qword ptr [rsp + 0x78]
    mov r14, qword ptr [rsp + 0x70]
    test rax, rax
    jne .egress_batch_failed

    mov qword ptr [rsp], 0x20
    lea rax, [rsp + 0x40]
    mov qword ptr [rsp + 8], rax
    mov qword ptr [rsp + 16], 42
    mov qword ptr [rsp + 24], 0
    lea rsi, [rip + .egress_batch_reply]
    lea rdi, [rsp + 0x40]
    mov ecx, 42
    rep movsb
    xor r13d, r13d

    # Fill the complete four-entry egress FIFO in sequence order.
.egress_batch_publish:
    mov r15, r12
    add r15, r13
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    mov r12, qword ptr [rsp + 0x78]
    mov r14, qword ptr [rsp + 0x70]
    cmp rax, r15
    jne .egress_batch_failed
    inc r13
    cmp r13, 4
    jb .egress_batch_publish

    mov esi, 64
    mov eax, 2
    int 0x80

.egress_batch_failed:
    int3

.egress_batch_reply:
    .byte 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02
    .byte 0x52, 0x54, 0x00, 0x12, 0x34, 0x56
    .byte 0x08, 0x00
    .byte 0x45, 0x00, 0x00, 0x1c, 0x12, 0x34, 0x40, 0x00
    .byte 0x40, 0x01, 0x10, 0x9d
    .byte 10, 0, 2, 15
    .byte 10, 0, 2, 2
    .byte 0x00, 0x00, 0x41, 0x0f, 0xbe, 0xef, 0x00, 0x01

ginux_packet_egress_batch_producer_end:

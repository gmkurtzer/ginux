.text
.balign 16
.global ginux_packet_protocol_loop_producer_start
.global ginux_packet_protocol_loop_producer_end

ginux_packet_protocol_loop_producer_start:
    mov r12, rbx
    mov r14, rcx
    sub rsp, 0x90
    mov qword ptr [rsp + 0x70], r12
    mov qword ptr [rsp + 0x78], r14
    mov qword ptr [rsp + 0x80], r8

    # Let the compiled read-only protocol service claim the first sequence.
    mov eax, 3
    int 0x80
    mov r12, qword ptr [rsp + 0x70]
    mov r14, qword ptr [rsp + 0x78]
    test rax, rax
    jne .protocol_loop_failed

    # Publish the first deterministic Ethernet/ARP reply.
    mov qword ptr [rsp], 0x20
    lea rax, [rsp + 0x40]
    mov qword ptr [rsp + 8], rax
    mov qword ptr [rsp + 16], 42
    mov qword ptr [rsp + 24], 0
    lea rsi, [rip + .protocol_loop_arp]
    lea rdi, [rsp + 0x40]
    mov ecx, 42
    rep movsb
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    mov r12, qword ptr [rsp + 0x70]
    mov r14, qword ptr [rsp + 0x78]
    cmp rax, r12
    jne .protocol_loop_failed

    # Give the service time to consume and block on the following sequence.
    mov eax, 3
    int 0x80
    mov r12, qword ptr [rsp + 0x70]
    mov r14, qword ptr [rsp + 0x78]
    test rax, rax
    jne .protocol_loop_failed
    inc r12
    mov qword ptr [rsp + 0x70], r12

    # Publish the second frame only after the consumer has yielded the CPU.
    lea rsi, [rip + .protocol_loop_arp]
    lea rdi, [rsp + 0x40]
    mov ecx, 42
    rep movsb
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    mov r12, qword ptr [rsp + 0x70]
    cmp rax, r12
    jne .protocol_loop_failed

    # A supervised proof requests one final scheduling point so the service can
    # consume and block on the following sequence before this producer exits.
    cmp qword ptr [rsp + 0x80], 0
    je .protocol_loop_complete
    mov eax, 3
    int 0x80
    mov r12, qword ptr [rsp + 0x70]
    mov r14, qword ptr [rsp + 0x78]
    test rax, rax
    jne .protocol_loop_failed

.protocol_loop_complete:
    mov esi, 54
    mov eax, 2
    int 0x80

.protocol_loop_failed:
    int3

.protocol_loop_arp:
    .byte 0x52, 0x54, 0x00, 0x12, 0x34, 0x56
    .byte 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02
    .byte 0x08, 0x06
    .byte 0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x02
    .byte 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02
    .byte 10, 0, 2, 2
    .byte 0x52, 0x54, 0x00, 0x12, 0x34, 0x56
    .byte 10, 0, 2, 15

ginux_packet_protocol_loop_producer_end:

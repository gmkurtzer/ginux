.text
.balign 16
.global ginux_packet_wait_producer_start
.global ginux_packet_wait_producer_end
.global ginux_packet_wait_consumer_start
.global ginux_packet_wait_consumer_end
.global ginux_packet_wait_stop_helper_start
.global ginux_packet_wait_stop_helper_end

ginux_packet_wait_producer_start:
    mov r12, rbx
    mov r14, rcx
    sub rsp, 0x90
    mov qword ptr [rsp + 0x70], r14
    mov qword ptr [rsp + 0x78], r12

    # Write-only producer authority cannot wait as a consumer.
    mov qword ptr [rsp], 0x20
    mov qword ptr [rsp + 8], 0
    mov qword ptr [rsp + 16], 0
    mov qword ptr [rsp + 24], r12
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 23
    int 0x80
    mov r12, qword ptr [rsp + 0x78]
    mov r14, qword ptr [rsp + 0x70]
    cmp rax, -3
    jne .producer_failed

    # Let the read-only peer reach its empty-queue wait first.
    mov eax, 3
    int 0x80
    mov r12, qword ptr [rsp + 0x78]
    mov r14, qword ptr [rsp + 0x70]
    cmp rax, 0
    jne .producer_failed

    # Publish one exact deterministic Ethernet/ARP reply.
    mov qword ptr [rsp], 0x20
    lea rax, [rsp + 0x40]
    mov qword ptr [rsp + 8], rax
    mov qword ptr [rsp + 16], 42
    mov qword ptr [rsp + 24], 0
    lea rsi, [rip + .producer_arp]
    lea rdi, [rsp + 0x40]
    mov ecx, 42
    rep movsb
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    cmp rax, r12
    jne .producer_failed

    mov esi, 48
    mov eax, 2
    int 0x80

.producer_failed:
    int3

.producer_arp:
    .byte 0x52, 0x54, 0x00, 0x12, 0x34, 0x56
    .byte 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02
    .byte 0x08, 0x06
    .byte 0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x02
    .byte 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02
    .byte 10, 0, 2, 2
    .byte 0x52, 0x54, 0x00, 0x12, 0x34, 0x56
    .byte 10, 0, 2, 15

ginux_packet_wait_producer_end:

.balign 16
ginux_packet_wait_consumer_start:
    mov r12, rbx
    mov r14, rcx
    sub rsp, 0x90
    mov qword ptr [rsp + 0x70], r14
    mov qword ptr [rsp + 0x78], r12

    # Wait requests carry no buffer and name the exact next sequence.
    mov qword ptr [rsp], 0x20
    mov qword ptr [rsp + 8], 0
    mov qword ptr [rsp + 16], 0
    lea r13, [r12 + 1]
    mov qword ptr [rsp + 24], r13

    # A future sequence is stale rather than an unbounded wait.
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 23
    int 0x80
    cmp rax, -5
    je .future_rejected
    mov esi, 1
    jmp .wait_consumer_failed
.future_rejected:

    # The exact empty-queue sequence blocks until publication wakes us.
    mov r12, qword ptr [rsp + 0x78]
    mov r14, qword ptr [rsp + 0x70]
    mov qword ptr [rsp + 24], r12
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 23
    int 0x80
    mov r12, qword ptr [rsp + 0x78]
    mov r14, qword ptr [rsp + 0x70]
    cmp rax, r12
    je .consumer_woken
    mov esi, eax
    and esi, 0xff
    jmp .wait_consumer_failed
.consumer_woken:

    # Consume only after the wake and verify the complete frame.
    lea rax, [rsp + 0x40]
    mov qword ptr [rsp + 8], rax
    mov qword ptr [rsp + 16], 64
    mov qword ptr [rsp + 24], r12
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 22
    int 0x80
    mov r12, qword ptr [rsp + 0x78]
    mov r14, qword ptr [rsp + 0x70]
    cmp rax, 42
    je .packet_consumed
    mov esi, 3
    jmp .wait_consumer_failed
.packet_consumed:
    lea rsi, [rip + .wait_consumer_arp]
    lea rdi, [rsp + 0x40]
    mov ecx, 42
    repe cmpsb
    je .packet_matched
    mov esi, 4
    jmp .wait_consumer_failed
.packet_matched:

    # The consumed sequence cannot be waited on again.
    mov qword ptr [rsp + 8], 0
    mov qword ptr [rsp + 16], 0
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 23
    int 0x80
    cmp rax, -5
    je .replay_rejected
    mov esi, 5
    jmp .wait_consumer_failed
.replay_rejected:

    mov esi, 49
    mov eax, 2
    int 0x80

.wait_consumer_failed:
    mov eax, 2
    int 0x80

.wait_consumer_arp:
    .byte 0x52, 0x54, 0x00, 0x12, 0x34, 0x56
    .byte 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02
    .byte 0x08, 0x06
    .byte 0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x02
    .byte 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02
    .byte 10, 0, 2, 2
    .byte 0x52, 0x54, 0x00, 0x12, 0x34, 0x56
    .byte 10, 0, 2, 15

ginux_packet_wait_consumer_end:

.balign 16
ginux_packet_wait_stop_helper_start:
    mov esi, 50
    mov eax, 2
    int 0x80
    int3
ginux_packet_wait_stop_helper_end:

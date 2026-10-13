.text
.balign 16
.global ginux_packet_egress_backpressure_producer_start
.global ginux_packet_egress_backpressure_producer_end
.global ginux_packet_egress_backpressure_consumer_start
.global ginux_packet_egress_backpressure_consumer_end

ginux_packet_egress_backpressure_producer_start:
    mov r12, rbx
    mov r14, rcx
    sub rsp, 0x80
    mov qword ptr [rsp], 0x20
    lea rax, [rsp + 0x40]
    mov qword ptr [rsp + 8], rax
    mov qword ptr [rsp + 16], 8
    mov qword ptr [rsp + 24], 0
    mov rax, 0x2121214543415053
    mov qword ptr [rsp + 0x40], rax
    xor r13d, r13d

.egress_backpressure_fill:
    mov r15, r12
    add r15, r13
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    cmp rax, r15
    jne .egress_backpressure_producer_failed
    inc r13
    cmp r13, 4
    jb .egress_backpressure_fill

    # The bounded egress FIFO rejects a fifth publication.
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    cmp rax, -3
    jne .egress_backpressure_producer_failed

    # Block on object 9 until its read-only peer frees one slot.
    mov rdi, r14
    xor esi, esi
    xor edx, edx
    mov eax, 29
    int 0x80
    test rax, rax
    jne .egress_backpressure_producer_failed

    # Retry publishes the exact next sequence into the released slot.
    mov r15, r12
    add r15, 4
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    cmp rax, r15
    jne .egress_backpressure_producer_failed

    mov esi, 60
    mov eax, 2
    int 0x80

.egress_backpressure_producer_failed:
    mov esi, eax
    and esi, 0xff
    mov eax, 2
    int 0x80
    int3
ginux_packet_egress_backpressure_producer_end:

.balign 16
ginux_packet_egress_backpressure_consumer_start:
    mov r12, rbx
    mov r14, rcx
    sub rsp, 0x80
    mov qword ptr [rsp], 0x20
    lea rax, [rsp + 0x40]
    mov qword ptr [rsp + 8], rax
    mov qword ptr [rsp + 16], 8

    # Read-only object-9 authority cannot wait for producer space.
    mov rdi, r14
    xor esi, esi
    xor edx, edx
    mov eax, 29
    int 0x80
    cmp rax, -3
    jne .egress_backpressure_consumer_failed

    # Removing the egress head wakes only its blocked producer.
    mov qword ptr [rsp + 24], r12
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 22
    int 0x80
    cmp rax, 8
    jne .egress_backpressure_consumer_failed
    mov rax, 0x2121214543415053
    cmp qword ptr [rsp + 0x40], rax
    jne .egress_backpressure_consumer_failed

    # Let the producer retry into the exact released slot and retire.
    mov eax, 3
    int 0x80
    test rax, rax
    jne .egress_backpressure_consumer_failed

    mov r13, 1
.egress_backpressure_drain:
    mov r15, r12
    add r15, r13
    mov qword ptr [rsp + 24], r15
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 22
    int 0x80
    cmp rax, 8
    jne .egress_backpressure_consumer_failed
    mov rax, 0x2121214543415053
    cmp qword ptr [rsp + 0x40], rax
    jne .egress_backpressure_consumer_failed
    inc r13
    cmp r13, 5
    jb .egress_backpressure_drain

    mov esi, 61
    mov eax, 2
    int 0x80

.egress_backpressure_consumer_failed:
    mov esi, eax
    and esi, 0xff
    mov eax, 2
    int 0x80
    int3
ginux_packet_egress_backpressure_consumer_end:

.text
.balign 16
.global ginux_packet_backpressure_producer_start
.global ginux_packet_backpressure_producer_end
.global ginux_packet_backpressure_consumer_start
.global ginux_packet_backpressure_consumer_end

ginux_packet_backpressure_producer_start:
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

.backpressure_fill:
    mov r15, r12
    add r15, r13
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    cmp rax, r15
    jne .backpressure_producer_failed
    inc r13
    cmp r13, 4
    jb .backpressure_fill

    # A fifth publication observes bounded FIFO backpressure.
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    cmp rax, -3
    jne .backpressure_producer_failed

    # Block without polling until the read-only consumer frees one slot.
    mov rdi, r14
    xor esi, esi
    xor edx, edx
    mov eax, 25
    int 0x80
    test rax, rax
    jne .backpressure_producer_failed

    # Retry publishes the exact next sequence into the released slot.
    mov r15, r12
    add r15, 4
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    cmp rax, r15
    jne .backpressure_producer_failed

    mov esi, 53
    mov eax, 2
    int 0x80

.backpressure_producer_failed:
    mov esi, eax
    and esi, 0xff
    mov eax, 2
    int 0x80
    int3
ginux_packet_backpressure_producer_end:

.balign 16
ginux_packet_backpressure_consumer_start:
    mov r12, rbx
    mov r14, rcx
    sub rsp, 0x80
    mov qword ptr [rsp], 0x20
    lea rax, [rsp + 0x40]
    mov qword ptr [rsp + 8], rax
    mov qword ptr [rsp + 16], 8

    # Read-only authority cannot claim producer-space notifications.
    mov rdi, r14
    xor esi, esi
    xor edx, edx
    mov eax, 25
    int 0x80
    cmp rax, -3
    jne .backpressure_consumer_failed

    # Removing the FIFO head wakes the sole blocked producer.
    mov qword ptr [rsp + 24], r12
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 22
    int 0x80
    cmp rax, 8
    jne .backpressure_consumer_failed
    mov rax, 0x2121214543415053
    cmp qword ptr [rsp + 0x40], rax
    jne .backpressure_consumer_failed

    # Let the producer retry into the exact released slot and retire.
    mov eax, 3
    int 0x80
    test rax, rax
    jne .backpressure_consumer_failed

    mov r13, 1
.backpressure_drain:
    mov r15, r12
    add r15, r13
    mov qword ptr [rsp + 24], r15
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 22
    int 0x80
    cmp rax, 8
    jne .backpressure_consumer_failed
    mov rax, 0x2121214543415053
    cmp qword ptr [rsp + 0x40], rax
    jne .backpressure_consumer_failed
    inc r13
    cmp r13, 5
    jb .backpressure_drain

    mov esi, 54
    mov eax, 2
    int 0x80

.backpressure_consumer_failed:
    mov esi, eax
    and esi, 0xff
    mov eax, 2
    int 0x80
    int3
ginux_packet_backpressure_consumer_end:

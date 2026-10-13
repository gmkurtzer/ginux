.text
.balign 16
.global ginux_packet_ipv4_producer_start
.global ginux_packet_ipv4_producer_end

ginux_packet_ipv4_producer_start:
    mov r12, rbx
    mov r14, rcx
    sub rsp, 0x90
    mov qword ptr [rsp + 0x70], r12
    mov qword ptr [rsp + 0x78], r14

    # Let the protocol service claim the first exact sequence.
    mov eax, 3
    int 0x80
    test rax, rax
    jne .ipv4_failed

    mov qword ptr [rsp], 0x20
    lea rax, [rsp + 0x40]
    mov qword ptr [rsp + 8], rax
    mov qword ptr [rsp + 16], 42
    mov qword ptr [rsp + 24], 0
    lea rsi, [rip + .arp_reply]
    lea rdi, [rsp + 0x40]
    mov ecx, 42
    rep movsb
    mov r12, qword ptr [rsp + 0x70]
    mov r14, qword ptr [rsp + 0x78]
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    cmp rax, r12
    jne .ipv4_failed

    # The service learns ARP state and blocks for the IPv4 sequence.
    mov eax, 3
    int 0x80
    test rax, rax
    jne .ipv4_failed
    inc r12
    mov qword ptr [rsp + 0x70], r12

    lea rsi, [rip + .ipv4_icmp]
    lea rdi, [rsp + 0x40]
    mov ecx, 42
    rep movsb
    mov r14, qword ptr [rsp + 0x78]
    mov rdi, r14
    lea rsi, [rsp]
    mov eax, 21
    int 0x80
    mov r12, qword ptr [rsp + 0x70]
    cmp rax, r12
    jne .ipv4_failed

    mov esi, 55
    mov eax, 2
    int 0x80

.ipv4_failed:
    int3

.arp_reply:
    .byte 0x52, 0x54, 0x00, 0x12, 0x34, 0x56
    .byte 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02
    .byte 0x08, 0x06
    .byte 0x00, 0x01, 0x08, 0x00, 0x06, 0x04, 0x00, 0x02
    .byte 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02
    .byte 10, 0, 2, 2
    .byte 0x52, 0x54, 0x00, 0x12, 0x34, 0x56
    .byte 10, 0, 2, 15

.ipv4_icmp:
    .byte 0x52, 0x54, 0x00, 0x12, 0x34, 0x56
    .byte 0x52, 0x55, 0x0a, 0x00, 0x02, 0x02
    .byte 0x08, 0x00
    .byte 0x45, 0x00, 0x00, 0x1c, 0x12, 0x34, 0x40, 0x00
    .byte 0x40, 0x01, 0x10, 0x9d, 10, 0, 2, 2, 10, 0, 2, 15
    .byte 0x08, 0x00, 0x39, 0x0f, 0xbe, 0xef, 0x00, 0x01

ginux_packet_ipv4_producer_end:

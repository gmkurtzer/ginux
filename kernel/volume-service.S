.text
.balign 16
.global ginux_volume_writer_start
.global ginux_volume_writer_fault
.global ginux_volume_writer_end
.global ginux_volume_reader_start
.global ginux_volume_reader_end

ginux_volume_writer_start:
    mov rdi, rcx
    sub rsp, 0x70

    # A write-only service cannot probe or read the volume. A denied read must
    # leave its output sentinel unchanged.
    xor edx, edx
    xor esi, esi
    mov eax, 18
    int 0x80
    cmp rax, -3
    jne .writer_failed
    movabs rax, 0x1122334455667788
    mov qword ptr [rsp + 0x40], rax
    movabs rax, 0x0000000400000020
    mov qword ptr [rsp], rax
    mov qword ptr [rsp + 8], 0
    lea rax, [rsp + 0x40]
    mov qword ptr [rsp + 16], rax
    mov qword ptr [rsp + 24], 64
    lea rdx, [rsp]
    mov esi, 2
    mov eax, 18
    int 0x80
    cmp rax, -3
    jne .writer_failed
    movabs rax, 0x1122334455667788
    cmp qword ptr [rsp + 0x40], rax
    jne .writer_failed

    # Commit "volume-service\n" as generation one.
    movabs rax, 0x732d656d756c6f76
    mov qword ptr [rsp + 0x40], rax
    movabs rax, 0x000a656369767265
    mov qword ptr [rsp + 0x48], rax
    movabs rax, 0x0000000400000020
    mov qword ptr [rsp], rax
    mov qword ptr [rsp + 8], 0
    lea rax, [rsp + 0x40]
    mov qword ptr [rsp + 16], rax
    mov qword ptr [rsp + 24], 15
    lea rdx, [rsp]
    mov esi, 3
    mov eax, 18
    int 0x80
    cmp rax, 1
    jne .writer_failed

    # Deliberately fault after the durable commit.
ginux_volume_writer_fault:
    ud2
.writer_failed:
    int3
ginux_volume_writer_end:

.balign 16
ginux_volume_reader_start:
    mov rdi, rcx
    sub rsp, 0x70

    # A read-only replacement discovers the contract.
    xor edx, edx
    xor esi, esi
    mov eax, 18
    int 0x80
    movabs rbx, 0x0000000200400001
    cmp rax, rbx
    jne .reader_failed

    # Read generation one while proving that byte 16 is not overwritten.
    mov qword ptr [rsp + 0x40], 0
    movabs rax, 0x5a00000000000000
    mov qword ptr [rsp + 0x48], rax
    movabs rax, 0x0000000400000020
    mov qword ptr [rsp], rax
    mov qword ptr [rsp + 8], 1
    lea rax, [rsp + 0x40]
    mov qword ptr [rsp + 16], rax
    mov qword ptr [rsp + 24], 64
    lea rdx, [rsp]
    mov esi, 2
    mov eax, 18
    int 0x80
    cmp rax, 15
    jne .reader_failed
    movabs rax, 0x732d656d756c6f76
    cmp qword ptr [rsp + 0x40], rax
    jne .reader_failed
    movabs rax, 0x5a0a656369767265
    cmp qword ptr [rsp + 0x48], rax
    jne .reader_failed

    # Query succeeds, mutation is denied, and the generation remains one.
    xor edx, edx
    mov esi, 1
    mov eax, 18
    int 0x80
    cmp rax, 1
    jne .reader_failed
    lea rdx, [rsp]
    mov esi, 3
    mov eax, 18
    int 0x80
    cmp rax, -3
    jne .reader_failed
    xor edx, edx
    mov esi, 1
    mov eax, 18
    int 0x80
    cmp rax, 1
    jne .reader_failed

    mov esi, 46
    mov eax, 2
    int 0x80
.reader_failed:
    int3
ginux_volume_reader_end:

.intel_syntax noprefix
.section .text
.global ginux_linux_interpreter
ginux_linux_interpreter:
    lea rbx, [rip]
    and rbx, -4096

    lea rdi, [rbx + 0x3008]
    mov ecx, 4
    xor eax, eax
.Lscan_interpreter_dynamic:
    cmp qword ptr [rdi], 7
    cmove rax, qword ptr [rdi + 8]
    add rdi, 16
    loop .Lscan_interpreter_dynamic
    test rax, rax
    jz .Linvalid_dynamic
    add rax, rbx
    mov rcx, qword ptr [rax]
    add rcx, rbx
    mov rdx, qword ptr [rax + 16]
    add rdx, rbx
    mov qword ptr [rcx], rdx

    mov edi, 0x404008
    mov ecx, 10
    xor eax, eax
    xor esi, esi
    xor r8d, r8d
.Lscan_main_dynamic:
    cmp qword ptr [rdi], 7
    cmove rax, qword ptr [rdi + 8]
    cmp qword ptr [rdi], 6
    cmove rsi, qword ptr [rdi + 8]
    cmp qword ptr [rdi], 4
    cmove r8, qword ptr [rdi + 8]
    add rdi, 16
    loop .Lscan_main_dynamic
    test rax, rax
    jz .Linvalid_dynamic
    test rsi, rsi
    jz .Linvalid_dynamic
    test r8, r8
    jz .Linvalid_dynamic

    mov edi, 0x613008
    mov ecx, 9
    xor r9d, r9d
    xor r10d, r10d
    xor r11d, r11d
.Lscan_dependency_dynamic:
    cmp qword ptr [rdi], 6
    cmove r9, qword ptr [rdi + 8]
    cmp qword ptr [rdi], 4
    cmove r10, qword ptr [rdi + 8]
    cmp qword ptr [rdi], 7
    cmove r11, qword ptr [rdi + 8]
    add rdi, 16
    loop .Lscan_dependency_dynamic
    test r9, r9
    jz .Linvalid_dynamic
    test r10, r10
    jz .Linvalid_dynamic
    test r11, r11
    jz .Linvalid_dynamic
    add r9, 0x610000
    add r10, 0x610000
    add r11, 0x610000

    mov rcx, qword ptr [r11]
    add rcx, 0x610000
    mov rdx, qword ptr [r11 + 16]
    add rdx, 0x610000
    mov qword ptr [rcx], rdx

    mov rcx, qword ptr [r11 + 24]
    add rcx, 0x610000
    mov rdx, qword ptr [r11 + 32]
    shr rdx, 32
    cmp edx, dword ptr [r10 + 8]
    jne .Linvalid_dynamic
    lea rdx, [rdx + rdx * 2]
    shl rdx, 3
    mov r12, qword ptr [r9 + rdx + 8]
    add r12, 0x610000
    add r12, qword ptr [r11 + 40]
    mov qword ptr [rcx], r12

    mov rcx, qword ptr [r11 + 48]
    add rcx, 0x610000
    mov rdx, qword ptr [r11 + 56]
    shr rdx, 32
    cmp edx, dword ptr [r10 + 8]
    jne .Linvalid_dynamic
    add r12, qword ptr [r11 + 64]
    mov qword ptr [rcx], r12

    mov rcx, qword ptr [rax]
    mov rdx, qword ptr [rax + 16]
    mov qword ptr [rcx], rdx

    mov rcx, qword ptr [rax + 24]
    mov rdx, qword ptr [rax + 32]
    shr rdx, 32
    cmp edx, dword ptr [r8 + 8]
    jne .Linvalid_dynamic
    cmp edx, dword ptr [r10 + 8]
    jne .Linvalid_dynamic
    lea rdx, [rdx + rdx * 2]
    shl rdx, 3
    mov rdx, qword ptr [r9 + rdx + 8]
    add rdx, 0x610000
    add rdx, qword ptr [rax + 40]
    mov qword ptr [rcx], rdx

    mov rcx, qword ptr [rax + 48]
    mov rdx, qword ptr [rax + 56]
    shr rdx, 32
    lea rdx, [rdx + rdx * 2]
    shl rdx, 3
    mov rdx, qword ptr [r9 + rdx + 8]
    add rdx, 0x610000
    add rdx, qword ptr [rax + 64]
    mov qword ptr [rcx], rdx

    mov eax, 0x400100
    jmp rax

.Linvalid_dynamic:
    ud2

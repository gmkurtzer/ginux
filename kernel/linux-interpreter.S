.intel_syntax noprefix
.section .text
.global ginux_linux_interpreter
ginux_linux_interpreter:
    lea rbx, [rip]
    and rbx, -4096

    lea rdi, [rbx + 0x3008]
    mov ecx, 17
    xor eax, eax
.Lscan_interpreter_dynamic:
    test qword ptr [rdi], -1
    jz .Linterpreter_dynamic_ready
    cmp qword ptr [rdi], 7
    cmove rax, qword ptr [rdi + 8]
    add rdi, 16
    loop .Lscan_interpreter_dynamic
    jmp .Linvalid_dynamic
.Linterpreter_dynamic_ready:
    test rax, rax
    jz .Linvalid_dynamic
    add rax, rbx
    mov rcx, qword ptr [rax]
    add rcx, rbx
    mov rdx, qword ptr [rax + 16]
    add rdx, rbx
    mov qword ptr [rcx], rdx

#ifdef GINUX_LINKED
    call .Ldiscover_main_file
#endif

    mov edi, 0x405200
    cmp qword ptr [rdi], 1
    je .Llinked_main_dynamic
    mov edi, 0x404008
    mov ecx, 13
    xor r15d, r15d
    jmp .Lmain_dynamic_ready
.Llinked_main_dynamic:
    mov ecx, 17
    mov r15d, 0x3ff000
.Lmain_dynamic_ready:
    xor ebx, ebx
    xor eax, eax
    xor esi, esi
    xor r8d, r8d
    xor r13d, r13d
    xor r12d, r12d
.Lscan_main_dynamic:
    cmp qword ptr [rdi], 7
    cmove rax, qword ptr [rdi + 8]
    cmp qword ptr [rdi], 6
    cmove rsi, qword ptr [rdi + 8]
    cmp qword ptr [rdi], 4
    cmove r8, qword ptr [rdi + 8]
    cmp qword ptr [rdi], 5
    cmove r13, qword ptr [rdi + 8]
    cmp qword ptr [rdi], 23
    cmove rbx, qword ptr [rdi + 8]
    cmp qword ptr [rdi], 1
    cmove r12, qword ptr [rdi + 8]
    add rdi, 16
    loop .Lscan_main_dynamic
    test rax, rax
    jz .Linvalid_dynamic
    test rsi, rsi
    jz .Linvalid_dynamic
    test r8, r8
    jz .Linvalid_dynamic
    test r13, r13
    jz .Linvalid_dynamic
    test rbx, rbx
    jz .Linvalid_dynamic
    test r12, r12
    jz .Linvalid_dynamic
    add rax, r15
    add rsi, r15
    add r8, r15
    add r13, r15
    add rbx, r15
    add r12, r13

#ifdef GINUX_LINKED
    push rax
    push rsi
    push r8
    push r13
    push rbx
    call .Ldiscover_dependency_file
    pop rbx
    pop r13
    pop r8
    pop rsi
    pop rax
#endif

    mov edi, 0x613000
    cmp qword ptr [rdi], 14
    je .Llinked_dependency_dynamic
    mov edi, 0x613008
    mov ecx, 13
    jmp .Ldependency_dynamic_ready
.Llinked_dependency_dynamic:
    mov ecx, 15
.Ldependency_dynamic_ready:
    xor r9d, r9d
    xor r10d, r10d
    xor r11d, r11d
    xor r14d, r14d
    xor ebp, ebp
.Lscan_dependency_dynamic:
    cmp qword ptr [rdi], 6
    cmove r9, qword ptr [rdi + 8]
    cmp qword ptr [rdi], 0x6ffffef5
    cmove r10, qword ptr [rdi + 8]
    cmp qword ptr [rdi], 7
    cmove r11, qword ptr [rdi + 8]
    cmp qword ptr [rdi], 5
    cmove r14, qword ptr [rdi + 8]
    cmp qword ptr [rdi], 23
    cmove rbp, qword ptr [rdi + 8]
    add rdi, 16
    loop .Lscan_dependency_dynamic
    test r9, r9
    jz .Linvalid_dynamic
    test r10, r10
    jz .Linvalid_dynamic
    test r11, r11
    jz .Linvalid_dynamic
    test r14, r14
    jz .Linvalid_dynamic
    test rbp, rbp
    jz .Linvalid_dynamic
    add r9, 0x610000
    add r10, 0x610000
    add r11, 0x610000
    add r14, 0x610000
    add rbp, 0x610000

    mov rcx, qword ptr [r11]
    add rcx, 0x610000
    mov rdx, qword ptr [r11 + 16]
    add rdx, 0x610000
    mov qword ptr [rcx], rdx

    mov rcx, qword ptr [r11 + 24]
    add rcx, 0x610000
    mov rdx, qword ptr [r11 + 32]
    shr rdx, 32
    cmp edx, dword ptr [r10 + 24]
    jne .Linvalid_dynamic
    lea rdx, [rdx + rdx * 2]
    shl rdx, 3
    mov r12, qword ptr [r9 + rdx + 8]
    add r12, 0x610000
    add r12, qword ptr [r11 + 40]
    mov qword ptr [rcx], r12

    mov rcx, qword ptr [rbp]
    add rcx, 0x610000
    mov rdx, qword ptr [rbp + 8]
    shr rdx, 32
    cmp edx, dword ptr [r10 + 24]
    jne .Linvalid_dynamic
    add r12, qword ptr [rbp + 16]
    mov qword ptr [rcx], r12

    mov rcx, qword ptr [rax]
    add rcx, r15
    mov rdx, qword ptr [rax + 16]
    add rdx, r15
    mov qword ptr [rcx], rdx

    mov rcx, qword ptr [rax + 24]
    add rcx, r15
    mov rdx, qword ptr [rax + 32]
    shr rdx, 32
    cmp edx, dword ptr [r8 + 8]
    jne .Linvalid_dynamic
    cmp edx, dword ptr [r10 + 24]
    jne .Linvalid_dynamic
    lea rdx, [rdx + rdx * 2]
    shl rdx, 3
    mov edi, dword ptr [rsi + rdx]
    add rdi, r13
    mov ebp, dword ptr [r9 + rdx]
    add rbp, r14
.Lcompare_symbol_name:
    mov r15b, byte ptr [rdi]
    cmp r15b, byte ptr [rbp]
    jne .Linvalid_dynamic
    inc rdi
    inc rbp
    test r15b, r15b
    jne .Lcompare_symbol_name
    mov rdx, qword ptr [r9 + rdx + 8]
    add rdx, 0x610000
    add rdx, qword ptr [rax + 40]
    mov qword ptr [rcx], rdx

    mov rcx, qword ptr [rbx]
    add rcx, r15
    mov rdx, qword ptr [rbx + 8]
    shr rdx, 32
    lea rdx, [rdx + rdx * 2]
    shl rdx, 3
    mov rdx, qword ptr [r9 + rdx + 8]
    add rdx, 0x610000
    add rdx, qword ptr [rbx + 16]
    mov qword ptr [rcx], rdx

    mov eax, 0x400100
    jmp rax

.Linvalid_dynamic:
    ud2

#ifdef GINUX_LINKED
.section .text.discovery,"ax",@progbits
.Ldiscover_main_file:
    lea rsi, [rip + .Lmain_path]
    call .Linspect_runtime_file

    mov edi, -100
    lea rsi, [rip + .Lmain_path]
    xor edx, edx
    mov eax, 257
    syscall
    cmp rax, 2
    jne .Linvalid_dynamic

    mov edi, 0x400000
    mov esi, 4096
    mov edx, 7
    mov r10d, 0x100002
    mov r8d, 2
    mov r9d, 0x1000
    mov eax, 9
    syscall
    cmp rax, -22
    jne .Linvalid_dynamic

    mov edi, 0x400000
    mov edx, 5
    mov r9d, 0x2000
    mov eax, 9
    syscall
    cmp rax, -22
    jne .Linvalid_dynamic

    mov edi, 0x610000
    mov r9d, 0x1000
    mov eax, 9
    syscall
    cmp rax, -22
    jne .Linvalid_dynamic

    mov edi, 0x3ff000
    mov edx, 1
    xor r9d, r9d
    call .Lmap_main_page
    mov eax, 9
    syscall
    cmp rax, -17
    jne .Linvalid_dynamic

    mov edi, 0x400000
    mov edx, 5
    mov r9d, 0x1000
    call .Lmap_main_page
    mov edi, 0x401000
    mov r9d, 0x2000
    call .Lmap_main_page
    mov edi, 0x402000
    mov r9d, 0x3000
    call .Lmap_main_page
    mov edi, 0x403000
    mov r9d, 0x4000
    call .Lmap_main_page

    mov edi, 0x404000
    mov edx, 3
    mov r9d, 0x5000
    call .Lmap_main_page
    mov edi, 0x405000
    mov r9d, 0x6000
    call .Lmap_main_page

    mov edi, 2
    mov eax, 3
    syscall
    test rax, rax
    jne .Linvalid_dynamic
    ret

.Lmap_main_page:
    mov esi, 4096
    mov r10d, 0x100002
    mov r8d, 2
    mov eax, 9
    syscall
    cmp rax, rdi
    jne .Linvalid_dynamic
    ret

.Ldiscover_dependency_file:
    sub rsp, 32
    movabs rax, 0x002f343662696c2f
    mov qword ptr [rsp], rax
    lea rdi, [rsp + 7]
    mov rsi, r12
    mov ecx, 13
.Lcopy_needed_name:
    lodsb
    stosb
    test al, al
    jz .Lneeded_path_ready
    loop .Lcopy_needed_name
    jmp .Linvalid_dynamic
.Lneeded_path_ready:
    lea rsi, [rsp]
    call .Linspect_runtime_file

    mov edi, -100
    lea rsi, [rsp]
    xor edx, edx
    mov eax, 257
    syscall
    cmp rax, 2
    jne .Linvalid_dynamic

    mov edi, 0x610000
    mov esi, 4096
    mov edx, 5
    mov r10d, 0x100002
    mov r8d, 2
    mov r9d, 0x1000
    mov eax, 9
    syscall
    cmp rax, 0x610000
    jne .Linvalid_dynamic
    mov eax, 9
    syscall
    cmp rax, -17
    jne .Linvalid_dynamic

    mov edi, 0x613000
    mov edx, 7
    mov r9d, 0x2000
    mov eax, 9
    syscall
    cmp rax, -22
    jne .Linvalid_dynamic
    mov edi, 0x613000
    mov edx, 3
    mov eax, 9
    syscall
    cmp rax, 0x613000
    jne .Linvalid_dynamic

    mov edi, 2
    mov eax, 3
    syscall
    test rax, rax
    jne .Linvalid_dynamic
    add rsp, 32
    ret

.Linspect_runtime_file:
    mov edi, -100
    xor edx, edx
    mov eax, 257
    syscall
    cmp rax, 2
    jne .Linvalid_dynamic

    xor edi, edi
    mov esi, 8192
    mov edx, 1
    mov r10d, 2
    mov r8d, 2
    xor r9d, r9d
    mov eax, 9
    syscall
    cmp rax, 0x40a000
    jne .Linvalid_dynamic
    cmp dword ptr [0x40a000], 0x464c457f
    jne .Linvalid_dynamic
    cmp byte ptr [0x40b100], 0x48
    jne .Linvalid_dynamic

    mov edi, 0x40a000
    mov esi, 8192
    mov eax, 11
    syscall
    test rax, rax
    jne .Linvalid_dynamic
    mov edi, 2
    mov eax, 3
    syscall
    test rax, rax
    jne .Linvalid_dynamic
    ret

.section .rodata.loader,"a",@progbits
.Lmain_path:
    .asciz "/init"

.section .data,"aw",@progbits
    .ascii "DYNAMIC!"

.section .data.rel.ro,"aw",@progbits
    .quad ginux_linux_interpreter
#endif

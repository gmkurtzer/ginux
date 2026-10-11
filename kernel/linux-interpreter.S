.intel_syntax noprefix
#ifdef GINUX_LINKED
#define GINUX_DEPENDENCY_BASE qword ptr [rip + .Ldependency_base]
#define GINUX_AUXILIARY_BASE qword ptr [rip + .Lauxiliary_dependency_base]
#else
#define GINUX_DEPENDENCY_BASE 0x610000
#endif
.section .text
.global ginux_linux_interpreter
ginux_linux_interpreter:
    lea rbx, [rip]
    and rbx, -4096

#ifdef GINUX_LINKED
    mov qword ptr [rip + .Linitial_stack], rsp
    cmp dword ptr [rbx], 0x464c457f
    jne .Linvalid_dynamic
    cmp word ptr [rbx + 54], 56
    jne .Linvalid_dynamic
    mov r12, qword ptr [rbx + 32]
    movzx r13d, word ptr [rbx + 56]
    test r13d, r13d
    jz .Linvalid_dynamic
    cmp r13d, 9
    ja .Linvalid_dynamic
    mov rax, r13
    imul rax, rax, 56
    add rax, r12
    jc .Linvalid_dynamic
    cmp rax, 4096
    ja .Linvalid_dynamic
    add r12, rbx
    xor r14d, r14d
    xor r15d, r15d
.Lscan_interpreter_headers:
    cmp dword ptr [r12], 2
    jne .Lnext_interpreter_header
    test r14, r14
    jnz .Linvalid_dynamic
    mov rax, qword ptr [r12 + 32]
    test rax, rax
    jz .Linvalid_dynamic
    cmp rax, qword ptr [r12 + 40]
    jne .Linvalid_dynamic
    test al, 15
    jnz .Linvalid_dynamic
    shr rax, 4
    cmp rax, 23
    ja .Linvalid_dynamic
    mov r14, qword ptr [r12 + 16]
    add r14, rbx
    jc .Linvalid_dynamic
    mov r15, rax
.Lnext_interpreter_header:
    add r12, 56
    dec r13d
    jnz .Lscan_interpreter_headers
    test r14, r14
    jz .Linvalid_dynamic
    test r15, r15
    jz .Linvalid_dynamic
    mov rdi, r14
    mov rcx, r15
#else
    lea rdi, [rbx + 0x3008]
    mov ecx, 17
#endif
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
    mov qword ptr [rip + .Lmain_phdr], 0
    mov qword ptr [rip + .Lmain_phnum], 0
    mov qword ptr [rip + .Lmain_entry], 0
    mov rdi, rsp
    mov rcx, qword ptr [rdi]
    cmp rcx, 8
    ja .Linvalid_dynamic
    lea rdi, [rdi + rcx * 8 + 16]
    mov ecx, 8
.Lscan_environment:
    cmp qword ptr [rdi], 0
    je .Lscan_auxiliary_vector
    add rdi, 8
    loop .Lscan_environment
    jmp .Linvalid_dynamic
.Lscan_auxiliary_vector:
    add rdi, 8
    mov ecx, 32
.Lscan_auxiliary_entry:
    mov rax, qword ptr [rdi]
    mov rdx, qword ptr [rdi + 8]
    test rax, rax
    jz .Lauxiliary_vector_ready
    cmp eax, 3
    jne .Lauxiliary_phnum
    mov qword ptr [rip + .Lmain_phdr], rdx
.Lauxiliary_phnum:
    cmp eax, 5
    jne .Lauxiliary_base
    mov qword ptr [rip + .Lmain_phnum], rdx
.Lauxiliary_base:
    cmp eax, 7
    jne .Lauxiliary_entry
    cmp rdx, rbx
    jne .Linvalid_dynamic
.Lauxiliary_entry:
    cmp eax, 9
    jne .Lnext_auxiliary_entry
    mov qword ptr [rip + .Lmain_entry], rdx
.Lnext_auxiliary_entry:
    add rdi, 16
    loop .Lscan_auxiliary_entry
    jmp .Linvalid_dynamic
.Lauxiliary_vector_ready:
    cmp qword ptr [rip + .Lmain_phdr], 0
    je .Linvalid_dynamic
    cmp qword ptr [rip + .Lmain_phnum], 0
    je .Linvalid_dynamic
    cmp qword ptr [rip + .Lmain_entry], 0
    je .Linvalid_dynamic
    call .Ldiscover_main_file
    jmp .Lmain_dynamic_ready
#endif

    mov edi, 0x406200
    cmp qword ptr [rdi], 1
    je .Llinked_main_dynamic
    mov edi, 0x405008
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
#ifdef GINUX_LINKED
    mov qword ptr [rip + .Lsecondary_needed], rax
    mov qword ptr [rip + .Lmain_relocation_size], rax
    mov qword ptr [rip + .Lmain_relocation_entry_size], rax
    mov qword ptr [rip + .Lmain_relative_count], rax
    mov qword ptr [rip + .Lmain_jump_relocation_size], rax
    mov qword ptr [rip + .Lmain_jump_relocation_kind], rax
    mov qword ptr [rip + .Lmain_preinit_array], rax
    mov qword ptr [rip + .Lmain_preinit_array_size], rax
    mov qword ptr [rip + .Lmain_init_function], rax
    mov qword ptr [rip + .Lmain_init_array], rax
    mov qword ptr [rip + .Lmain_init_array_size], rax
#endif
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
#ifdef GINUX_LINKED
    cmp qword ptr [rdi], 8
    jne .Lscan_main_relocation_entry_size
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lmain_relocation_size], rdx
.Lscan_main_relocation_entry_size:
    cmp qword ptr [rdi], 9
    jne .Lscan_main_relative_count
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lmain_relocation_entry_size], rdx
.Lscan_main_relative_count:
    cmp qword ptr [rdi], 0x6ffffff9
    jne .Lscan_main_jump_relocation_size
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lmain_relative_count], rdx
.Lscan_main_jump_relocation_size:
    cmp qword ptr [rdi], 2
    jne .Lscan_main_jump_relocation_kind
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lmain_jump_relocation_size], rdx
.Lscan_main_jump_relocation_kind:
    cmp qword ptr [rdi], 20
    jne .Lscan_main_preinit_array
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lmain_jump_relocation_kind], rdx
.Lscan_main_preinit_array:
    cmp qword ptr [rdi], 32
    jne .Lscan_main_preinit_array_size
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lmain_preinit_array], rdx
.Lscan_main_preinit_array_size:
    cmp qword ptr [rdi], 33
    jne .Lscan_main_init_function
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lmain_preinit_array_size], rdx
.Lscan_main_init_function:
    cmp qword ptr [rdi], 12
    jne .Lscan_main_init_array
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lmain_init_function], rdx
.Lscan_main_init_array:
    cmp qword ptr [rdi], 25
    jne .Lscan_main_init_array_size
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lmain_init_array], rdx
.Lscan_main_init_array_size:
    cmp qword ptr [rdi], 27
    jne .Lscan_main_needed
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lmain_init_array_size], rdx
.Lscan_main_needed:
#endif
    cmp qword ptr [rdi], 1
    jne .Lnext_main_dynamic
    test r12, r12
    jnz .Lrecord_secondary_needed
    mov r12, qword ptr [rdi + 8]
    jmp .Lnext_main_dynamic
.Lrecord_secondary_needed:
#ifdef GINUX_LINKED
    cmp qword ptr [rip + .Lsecondary_needed], 0
    jne .Linvalid_dynamic
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lsecondary_needed], rdx
#else
    jmp .Linvalid_dynamic
#endif
.Lnext_main_dynamic:
    add rdi, 16
    dec rcx
    jnz .Lscan_main_dynamic
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
#ifdef GINUX_LINKED
    cmp qword ptr [rip + .Lsecondary_needed], 0
    je .Linvalid_dynamic
    cmp qword ptr [rip + .Lmain_relocation_size], 96
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lmain_relocation_entry_size], 24
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lmain_relative_count], 2
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lmain_jump_relocation_size], 48
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lmain_jump_relocation_kind], 7
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lmain_preinit_array], 0
    je .Linvalid_dynamic
    cmp qword ptr [rip + .Lmain_preinit_array_size], 8
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lmain_init_function], 0
    je .Linvalid_dynamic
    cmp qword ptr [rip + .Lmain_init_array], 0
    je .Linvalid_dynamic
    cmp qword ptr [rip + .Lmain_init_array_size], 8
    jne .Linvalid_dynamic
#endif
    add rax, r15
    add rsi, r15
    add r8, r15
    add r13, r15
    add rbx, r15
    add r12, r13

#ifdef GINUX_LINKED
    mov rdx, qword ptr [rip + .Lsecondary_needed]
    add rdx, r13
    jc .Linvalid_dynamic
    mov qword ptr [rip + .Lauxiliary_name], rdx
    push rax
    push rsi
    push r8
    push r13
    push rbx
    mov rsi, qword ptr fs:[0]
    sub rsi, 16
    mov edi, 1
    mov edx, 8
    mov eax, 1
    syscall
    cmp rax, 8
    jne .Linvalid_dynamic
    mov rax, qword ptr [rsp + 32]
    mov rcx, qword ptr [rax]
    add rcx, r15
    mov rdx, qword ptr [rip + .Lmain_preinit_array]
    add rdx, r15
    cmp rcx, rdx
    jne .Linvalid_dynamic
    mov rdx, qword ptr [rax + 16]
    add rdx, r15
    mov qword ptr [rcx], rdx
    mov rcx, qword ptr [rcx]
    call .Lcall_initializer
    call .Ldiscover_dependency_file
    mov qword ptr [rip + .Lprimary_dynamic_address], rdi
    mov qword ptr [rip + .Lprimary_dynamic_count], rcx
    mov r12, qword ptr [rip + .Lauxiliary_name]
    call .Ldiscover_auxiliary_file
    call .Lrelocate_auxiliary
    mov rdi, qword ptr [rip + .Lprimary_dynamic_address]
    mov rcx, qword ptr [rip + .Lprimary_dynamic_count]
    pop rbx
    pop r13
    pop r8
    pop rsi
    pop rax
    jmp .Ldependency_dynamic_ready
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
#ifdef GINUX_LINKED
    mov qword ptr [rip + .Lprimary_init_function], r9
    mov qword ptr [rip + .Lprimary_relocation_size], r9
    mov qword ptr [rip + .Lprimary_relocation_entry_size], r9
    mov qword ptr [rip + .Lprimary_relative_count], r9
    mov qword ptr [rip + .Lprimary_jump_relocation_size], r9
    mov qword ptr [rip + .Lprimary_jump_relocation_kind], r9
    mov qword ptr [rip + .Lprimary_flags], r9
    mov qword ptr [rip + .Lprimary_init_array], r9
    mov qword ptr [rip + .Lprimary_init_array_size], r9
#endif
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
#ifdef GINUX_LINKED
    cmp qword ptr [rdi], 8
    jne .Lscan_primary_relocation_entry_size
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lprimary_relocation_size], rdx
.Lscan_primary_relocation_entry_size:
    cmp qword ptr [rdi], 9
    jne .Lscan_primary_relative_count
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lprimary_relocation_entry_size], rdx
.Lscan_primary_relative_count:
    cmp qword ptr [rdi], 0x6ffffff9
    jne .Lscan_primary_jump_relocation_size
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lprimary_relative_count], rdx
.Lscan_primary_jump_relocation_size:
    cmp qword ptr [rdi], 2
    jne .Lscan_primary_jump_relocation_kind
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lprimary_jump_relocation_size], rdx
.Lscan_primary_jump_relocation_kind:
    cmp qword ptr [rdi], 20
    jne .Lscan_primary_flags
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lprimary_jump_relocation_kind], rdx
.Lscan_primary_flags:
    cmp qword ptr [rdi], 30
    jne .Lscan_primary_init_function
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lprimary_flags], rdx
.Lscan_primary_init_function:
    cmp qword ptr [rdi], 12
    jne .Lscan_primary_init_array
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lprimary_init_function], rdx
.Lscan_primary_init_array:
    cmp qword ptr [rdi], 25
    jne .Lscan_primary_init_array_size
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lprimary_init_array], rdx
.Lscan_primary_init_array_size:
    cmp qword ptr [rdi], 27
    jne .Ladvance_dependency_dynamic
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lprimary_init_array_size], rdx
.Ladvance_dependency_dynamic:
#endif
    add rdi, 16
    dec rcx
    jnz .Lscan_dependency_dynamic
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
#ifdef GINUX_LINKED
    cmp qword ptr [rip + .Lprimary_init_function], 0
    je .Linvalid_dynamic
    cmp qword ptr [rip + .Lprimary_relocation_size], 72
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lprimary_relocation_entry_size], 24
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lprimary_relative_count], 1
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lprimary_jump_relocation_size], 24
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lprimary_jump_relocation_kind], 7
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lprimary_flags], 16
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lprimary_init_array], 0
    je .Linvalid_dynamic
    cmp qword ptr [rip + .Lprimary_init_array_size], 8
    jne .Linvalid_dynamic
#endif
    add r9, GINUX_DEPENDENCY_BASE
    add r10, GINUX_DEPENDENCY_BASE
    add r11, GINUX_DEPENDENCY_BASE
    add r14, GINUX_DEPENDENCY_BASE
    add rbp, GINUX_DEPENDENCY_BASE

    mov rcx, qword ptr [r11]
    add rcx, GINUX_DEPENDENCY_BASE
#ifdef GINUX_LINKED
    mov rdx, qword ptr [rip + .Lprimary_init_array]
    add rdx, GINUX_DEPENDENCY_BASE
    cmp rcx, rdx
    jne .Linvalid_dynamic
#endif
    mov rdx, qword ptr [r11 + 16]
    add rdx, GINUX_DEPENDENCY_BASE
    mov qword ptr [rcx], rdx

    mov rcx, qword ptr [r11 + 24]
    add rcx, GINUX_DEPENDENCY_BASE
    mov rdx, qword ptr [r11 + 32]
#ifdef GINUX_LINKED
    cmp edx, 18
    jne .Linvalid_dynamic
    shr rdx, 32
    test rdx, rdx
    jne .Linvalid_dynamic
    cmp qword ptr [r11 + 40], 0
    jne .Linvalid_dynamic
    mov qword ptr [rcx], -32

    mov rcx, qword ptr [r11 + 48]
    add rcx, GINUX_DEPENDENCY_BASE
    mov rdx, qword ptr [r11 + 56]
#endif
    shr rdx, 32
    cmp edx, dword ptr [r10 + 24]
    jne .Linvalid_dynamic
    lea rdx, [rdx + rdx * 2]
    shl rdx, 3
    mov r12, qword ptr [r9 + rdx + 8]
    add r12, GINUX_DEPENDENCY_BASE
#ifdef GINUX_LINKED
    add r12, qword ptr [r11 + 64]
#else
    add r12, qword ptr [r11 + 40]
#endif
    mov qword ptr [rcx], r12

    mov rcx, qword ptr [rbp]
    add rcx, GINUX_DEPENDENCY_BASE
    mov rdx, qword ptr [rbp + 8]
    shr rdx, 32
    cmp edx, dword ptr [r10 + 24]
    jne .Linvalid_dynamic
    add r12, qword ptr [rbp + 16]
    mov qword ptr [rcx], r12

#ifdef GINUX_LINKED
    push rax
    push rsi
    mov rcx, qword ptr [rip + .Lprimary_init_function]
    add rcx, GINUX_DEPENDENCY_BASE
    call .Lcall_initializer
    mov rcx, qword ptr [rip + .Lprimary_init_array]
    add rcx, GINUX_DEPENDENCY_BASE
    mov rcx, qword ptr [rcx]
    call .Lcall_initializer
    pop rsi
    pop rax
#endif

    mov rcx, qword ptr [rax + 24]
    add rcx, r15
#ifdef GINUX_LINKED
    mov rdx, qword ptr [rip + .Lmain_init_array]
    add rdx, r15
    cmp rcx, rdx
    jne .Linvalid_dynamic
#endif
    mov rdx, qword ptr [rax + 40]
    add rdx, r15
    mov qword ptr [rcx], rdx

    mov rcx, qword ptr [rax + 48]
    add rcx, r15
    mov rdx, qword ptr [rax + 56]
    shr rdx, 32
#ifdef GINUX_LINKED
    cmp edx, dword ptr [r8 + 12]
#else
    cmp edx, dword ptr [r8 + 8]
#endif
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
    add rdx, GINUX_DEPENDENCY_BASE
    add rdx, qword ptr [rax + 64]
    mov qword ptr [rcx], rdx

    mov rcx, qword ptr [rbx]
    add rcx, r15
    mov rdx, qword ptr [rbx + 8]
    shr rdx, 32
    lea rdx, [rdx + rdx * 2]
    shl rdx, 3
    mov rdx, qword ptr [r9 + rdx + 8]
    add rdx, GINUX_DEPENDENCY_BASE
    add rdx, qword ptr [rbx + 16]
    mov qword ptr [rcx], rdx

#ifdef GINUX_LINKED
    mov rcx, qword ptr [rax + 72]
    add rcx, r15
    mov rdx, qword ptr [rax + 80]
    shr rdx, 32
    cmp edx, dword ptr [r8 + 8]
    jne .Linvalid_dynamic
    cmp edx, 2
    jne .Linvalid_dynamic
    lea rdx, [rdx + rdx * 2]
    shl rdx, 3
    mov edi, dword ptr [rsi + rdx]
    add rdi, r13
    mov r9, qword ptr [rip + .Lauxiliary_symbol_table]
    mov r10, qword ptr [rip + .Lauxiliary_hash_table]
    mov r14, qword ptr [rip + .Lauxiliary_string_table]
    mov edx, dword ptr [r10 + 24]
    cmp edx, 1
    jne .Linvalid_dynamic
    lea rdx, [rdx + rdx * 2]
    shl rdx, 3
    mov ebp, dword ptr [r9 + rdx]
    add rbp, r14
.Lcompare_auxiliary_symbol_name:
    mov r12b, byte ptr [rdi]
    cmp r12b, byte ptr [rbp]
    jne .Linvalid_dynamic
    inc rdi
    inc rbp
    test r12b, r12b
    jne .Lcompare_auxiliary_symbol_name
    mov rdx, qword ptr [r9 + rdx + 8]
    add rdx, GINUX_AUXILIARY_BASE
    add rdx, qword ptr [rax + 88]
    mov qword ptr [rcx], rdx

    mov rcx, qword ptr [rbx + 24]
    add rcx, r15
    mov rdi, qword ptr [rbx + 32]
    shr rdi, 32
    cmp edi, 2
    jne .Linvalid_dynamic
    add rdx, qword ptr [rbx + 40]
    mov qword ptr [rcx], rdx
#endif

#ifdef GINUX_LINKED
    mov rcx, qword ptr [rip + .Lmain_init_function]
    add rcx, r15
    call .Lcall_initializer
    mov rcx, qword ptr [rip + .Lmain_init_array]
    add rcx, r15
    mov rcx, qword ptr [rcx]
    call .Lcall_initializer
    mov rax, qword ptr [rip + .Lmain_entry]
#else
    mov eax, 0x400100
#endif
    jmp rax

#ifdef GINUX_LINKED
.Lcall_initializer:
    mov rax, qword ptr [rip + .Linitial_stack]
    mov rdi, qword ptr [rax]
    lea rsi, [rax + 8]
    lea rdx, [rsi + rdi * 8 + 8]
    push rax
    call rcx
    pop rax
    ret
#endif

.Linvalid_dynamic:
    ud2

#ifdef GINUX_LINKED
.section .text.discovery,"ax",@progbits
.Ldiscover_main_file:
    call .Linspect_root_hostname
    lea rsi, [rip + .Lmain_path]
    call .Linspect_runtime_file
    lea rsi, [rip + .Lmain_path]
    mov r14, qword ptr [rip + .Lmain_phdr]
    mov rax, qword ptr [rip + .Lmain_phnum]
    mov qword ptr [rip + .Lexpected_phnum], rax
    call .Lload_runtime_file
    mov r15, r14
    ret

.Linspect_root_hostname:
    sub rsp, 160
    mov edi, -100
    lea rsi, [rip + .Lhostname_path]
    xor edx, edx
    mov eax, 257
    syscall
    cmp rax, 2
    jne .Linvalid_dynamic

    mov edi, 2
    mov rsi, rsp
    mov edx, 6
    xor eax, eax
    syscall
    cmp rax, 6
    jne .Linvalid_dynamic
    cmp dword ptr [rsp], 0x756e6967
    jne .Linvalid_dynamic
    cmp word ptr [rsp + 4], 0x0a78
    jne .Linvalid_dynamic

    mov edi, 2
    lea rsi, [rsp + 16]
    mov eax, 5
    syscall
    test rax, rax
    jne .Linvalid_dynamic
    cmp dword ptr [rsp + 40], 0x816d
    jne .Linvalid_dynamic
    cmp qword ptr [rsp + 64], 6
    jne .Linvalid_dynamic

    mov edi, 2
    xor esi, esi
    mov edx, 2
    mov eax, 8
    syscall
    cmp rax, 6
    jne .Linvalid_dynamic

    mov edi, 2
    mov rsi, rsp
    mov edx, 1
    mov eax, 1
    syscall
    cmp rax, -9
    jne .Linvalid_dynamic

    mov edi, 2
    mov eax, 3
    syscall
    test rax, rax
    jne .Linvalid_dynamic

    mov edi, -100
    lea rsi, [rip + .Lhostname_path]
    mov edx, 1
    mov eax, 257
    syscall
    cmp rax, -13
    jne .Linvalid_dynamic
    add rsp, 160
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
    lea rsi, [rsp]
    mov r14d, 0x618000
    mov qword ptr [rip + .Ldependency_base], r14
    mov qword ptr [rip + .Lexpected_phnum], 0
    call .Lload_runtime_file
    push rdi
    push rcx
    lea rsi, [rsp + 16]
    mov edi, -100
    xor edx, edx
    mov eax, 257
    syscall
    cmp rax, 2
    jne .Linvalid_dynamic
    mov edi, 0x61c000
    mov esi, 4096
    mov edx, 3
    mov r10d, 0x100002
    mov r8d, 2
    mov r9d, 0x4000
    mov eax, 9
    syscall
    cmp rax, -22
    jne .Linvalid_dynamic
    mov edi, 2
    mov eax, 3
    syscall
    test rax, rax
    jne .Linvalid_dynamic
    pop rcx
    pop rdi
    add rsp, 32
    ret

.Ldiscover_auxiliary_file:
    sub rsp, 32
    movabs rax, 0x002f343662696c2f
    mov qword ptr [rsp], rax
    lea rdi, [rsp + 7]
    mov rsi, r12
    mov ecx, 24
.Lcopy_auxiliary_name:
    lodsb
    stosb
    test al, al
    jz .Lauxiliary_path_ready
    loop .Lcopy_auxiliary_name
    jmp .Linvalid_dynamic
.Lauxiliary_path_ready:
    lea rsi, [rsp]
    call .Linspect_runtime_file
    lea rsi, [rsp]
    mov r14d, 0x638000
    mov qword ptr [rip + .Lauxiliary_dependency_base], r14
    mov qword ptr [rip + .Lexpected_phnum], 0
    call .Lload_runtime_file
    add rsp, 32
    ret

.Lrelocate_auxiliary:
    xor r9d, r9d
    xor r10d, r10d
    xor r11d, r11d
    xor r14d, r14d
    xor ebp, ebp
    mov qword ptr [rip + .Lauxiliary_init_array], r9
    mov qword ptr [rip + .Lauxiliary_init_array_size], r9
    mov qword ptr [rip + .Lauxiliary_init_function], r9
    mov qword ptr [rip + .Lauxiliary_relocation_size], r9
    mov qword ptr [rip + .Lauxiliary_relocation_entry_size], r9
    mov qword ptr [rip + .Lauxiliary_relative_count], r9
    mov qword ptr [rip + .Lauxiliary_jump_relocation_size], r9
    mov qword ptr [rip + .Lauxiliary_jump_relocation_kind], r9
    mov qword ptr [rip + .Lauxiliary_flags], r9
.Lscan_auxiliary_dynamic:
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
    cmp qword ptr [rdi], 8
    jne .Lscan_auxiliary_relocation_entry_size
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lauxiliary_relocation_size], rdx
.Lscan_auxiliary_relocation_entry_size:
    cmp qword ptr [rdi], 9
    jne .Lscan_auxiliary_relative_count
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lauxiliary_relocation_entry_size], rdx
.Lscan_auxiliary_relative_count:
    cmp qword ptr [rdi], 0x6ffffff9
    jne .Lscan_auxiliary_jump_relocation_size
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lauxiliary_relative_count], rdx
.Lscan_auxiliary_jump_relocation_size:
    cmp qword ptr [rdi], 2
    jne .Lscan_auxiliary_jump_relocation_kind
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lauxiliary_jump_relocation_size], rdx
.Lscan_auxiliary_jump_relocation_kind:
    cmp qword ptr [rdi], 20
    jne .Lscan_auxiliary_flags
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lauxiliary_jump_relocation_kind], rdx
.Lscan_auxiliary_flags:
    cmp qword ptr [rdi], 30
    jne .Lscan_auxiliary_init_function
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lauxiliary_flags], rdx
.Lscan_auxiliary_init_function:
    cmp qword ptr [rdi], 12
    jne .Lscan_auxiliary_init_array
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lauxiliary_init_function], rdx
.Lscan_auxiliary_init_array:
    cmp qword ptr [rdi], 25
    jne .Lscan_auxiliary_init_array_size
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lauxiliary_init_array], rdx
.Lscan_auxiliary_init_array_size:
    cmp qword ptr [rdi], 27
    jne .Ladvance_auxiliary_dynamic
    mov rdx, qword ptr [rdi + 8]
    mov qword ptr [rip + .Lauxiliary_init_array_size], rdx
.Ladvance_auxiliary_dynamic:
    add rdi, 16
    dec rcx
    jnz .Lscan_auxiliary_dynamic
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
    cmp qword ptr [rip + .Lauxiliary_init_function], 0
    je .Linvalid_dynamic
    cmp qword ptr [rip + .Lauxiliary_relocation_size], 72
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lauxiliary_relocation_entry_size], 24
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lauxiliary_relative_count], 1
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lauxiliary_jump_relocation_size], 24
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lauxiliary_jump_relocation_kind], 7
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lauxiliary_flags], 16
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lauxiliary_init_array], 0
    je .Linvalid_dynamic
    cmp qword ptr [rip + .Lauxiliary_init_array_size], 8
    jne .Linvalid_dynamic
    add r9, GINUX_AUXILIARY_BASE
    add r10, GINUX_AUXILIARY_BASE
    add r11, GINUX_AUXILIARY_BASE
    add r14, GINUX_AUXILIARY_BASE
    add rbp, GINUX_AUXILIARY_BASE
    mov qword ptr [rip + .Lauxiliary_symbol_table], r9
    mov qword ptr [rip + .Lauxiliary_hash_table], r10
    mov qword ptr [rip + .Lauxiliary_string_table], r14

    mov rcx, qword ptr [r11]
    add rcx, GINUX_AUXILIARY_BASE
    mov rdx, qword ptr [rip + .Lauxiliary_init_array]
    add rdx, GINUX_AUXILIARY_BASE
    cmp rcx, rdx
    jne .Linvalid_dynamic
    mov rdx, qword ptr [r11 + 16]
    add rdx, GINUX_AUXILIARY_BASE
    mov qword ptr [rcx], rdx

    mov rcx, qword ptr [r11 + 24]
    add rcx, GINUX_AUXILIARY_BASE
    mov rdx, qword ptr [r11 + 32]
    cmp edx, 18
    jne .Linvalid_dynamic
    shr rdx, 32
    test rdx, rdx
    jne .Linvalid_dynamic
    cmp qword ptr [r11 + 40], 0
    jne .Linvalid_dynamic
    mov qword ptr [rcx], -48

    mov rcx, qword ptr [r11 + 48]
    add rcx, GINUX_AUXILIARY_BASE
    mov rdx, qword ptr [r11 + 56]
    shr rdx, 32
    cmp edx, dword ptr [r10 + 24]
    jne .Linvalid_dynamic
    lea rdx, [rdx + rdx * 2]
    shl rdx, 3
    mov r12, qword ptr [r9 + rdx + 8]
    add r12, GINUX_AUXILIARY_BASE
    add r12, qword ptr [r11 + 64]
    mov qword ptr [rcx], r12

    mov rcx, qword ptr [rbp]
    add rcx, GINUX_AUXILIARY_BASE
    mov rdx, qword ptr [rbp + 8]
    shr rdx, 32
    cmp edx, dword ptr [r10 + 24]
    jne .Linvalid_dynamic
    add r12, qword ptr [rbp + 16]
    mov qword ptr [rcx], r12

    mov rcx, qword ptr [rip + .Lauxiliary_init_function]
    add rcx, GINUX_AUXILIARY_BASE
    call .Lcall_initializer

    mov rcx, qword ptr [rip + .Lauxiliary_init_array]
    add rcx, GINUX_AUXILIARY_BASE
    mov rcx, qword ptr [rcx]
    call .Lcall_initializer

    mov rax, GINUX_AUXILIARY_BASE
    add rax, 0x100
    call rax
    movabs rdx, 0x5952414958554147
    cmp rax, rdx
    jne .Linvalid_dynamic
    ret

.Lload_runtime_file:
    push r15
    xor eax, eax
    mov qword ptr [rip + .Lloaded_dynamic_address], rax
    mov qword ptr [rip + .Lloaded_dynamic_count], rax
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
    cmp rax, 0x40b000
    jne .Linvalid_dynamic
    cmp dword ptr [0x40b000], 0x464c457f
    jne .Linvalid_dynamic

    mov r12, qword ptr [0x40b020]
    movzx r13d, word ptr [0x40b038]
    cmp word ptr [0x40b036], 56
    jne .Linvalid_dynamic
    test r13d, r13d
    jz .Linvalid_dynamic
    cmp r13d, 9
    ja .Linvalid_dynamic
    mov rax, qword ptr [rip + .Lexpected_phnum]
    test rax, rax
    jz .Lprogram_header_count_ready
    cmp r13, rax
    jne .Linvalid_dynamic
    sub r14, r12
    jc .Linvalid_dynamic
.Lprogram_header_count_ready:
    mov rax, r13
    imul rax, rax, 56
    add rax, r12
    jc .Linvalid_dynamic
    cmp rax, 8192
    ja .Linvalid_dynamic
    add r12, 0x40b000
    xor ebp, ebp
    xor r15d, r15d

.Lnext_program_header:
    cmp dword ptr [r12], 1
    je .Lload_program_header
    cmp dword ptr [r12], 2
    jne .Ladvance_program_header
    cmp qword ptr [rip + .Lloaded_dynamic_address], 0
    jne .Linvalid_dynamic
    mov rax, qword ptr [r12 + 32]
    test rax, rax
    jz .Linvalid_dynamic
    cmp rax, qword ptr [r12 + 40]
    jne .Linvalid_dynamic
    test al, 15
    jnz .Linvalid_dynamic
    shr rax, 4
    cmp rax, 23
    ja .Linvalid_dynamic
    mov rcx, qword ptr [r12 + 16]
    add rcx, r14
    jc .Linvalid_dynamic
    mov qword ptr [rip + .Lloaded_dynamic_address], rcx
    mov qword ptr [rip + .Lloaded_dynamic_count], rax
    jmp .Ladvance_program_header

.Lload_program_header:
    mov eax, dword ptr [r12 + 4]
    test eax, -8
    jnz .Linvalid_dynamic
    test eax, 4
    jz .Linvalid_dynamic
    mov rbx, qword ptr [r12 + 40]
    test rbx, rbx
    jz .Linvalid_dynamic
    mov rcx, qword ptr [r12 + 32]
    cmp rcx, rbx
    ja .Linvalid_dynamic

    mov r9, qword ptr [r12 + 8]
    mov rcx, qword ptr [r12 + 16]
    mov rdx, r9
    xor rdx, rcx
    test edx, 4095
    jnz .Linvalid_dynamic
    and r9, -4096
    add rcx, r14
    jc .Linvalid_dynamic
    mov rdi, rcx
    and rdi, -4096
    and ecx, 4095
    add rbx, rcx
    jc .Linvalid_dynamic
    add rbx, 4095
    jc .Linvalid_dynamic
    shr rbx, 12
    test rbx, rbx
    jz .Linvalid_dynamic
    cmp rbx, 5
    ja .Linvalid_dynamic

    xor edx, edx
    test eax, 4
    jz .Lload_write_flag
    or edx, 1
.Lload_write_flag:
    test eax, 2
    jz .Lload_execute_flag
    or edx, 2
.Lload_execute_flag:
    test eax, 1
    jz .Lmap_load_page
    or edx, 4

.Lmap_load_page:
    cmp ebp, 8
    jae .Linvalid_dynamic
    mov esi, 4096
    mov r10d, 0x100002
    mov r8d, 2
    test ebp, ebp
    jnz .Ltest_load_wx
    add r9, 32768
    mov eax, 9
    syscall
    cmp rax, -22
    jne .Linvalid_dynamic
    sub r9, 32768
.Ltest_load_wx:
    test edx, 4
    jz .Linstall_load_page
    test r15d, r15d
    jnz .Linstall_load_page
    or edx, 2
    mov eax, 9
    syscall
    cmp rax, -22
    jne .Linvalid_dynamic
    xor edx, 2
    mov r15d, 1
.Linstall_load_page:
    mov eax, 9
    syscall
    cmp rax, rdi
    jne .Linvalid_dynamic
    test ebp, ebp
    jnz .Lload_page_installed
    mov eax, 9
    syscall
    cmp rax, -17
    jne .Linvalid_dynamic
.Lload_page_installed:
    inc ebp
    add rdi, 4096
    add r9, 4096
    dec rbx
    jnz .Lmap_load_page

.Ladvance_program_header:
    add r12, 56
    dec r13d
    jnz .Lnext_program_header
    test ebp, ebp
    jz .Linvalid_dynamic
    cmp r15d, 1
    jne .Linvalid_dynamic
    cmp qword ptr [rip + .Lloaded_dynamic_address], 0
    je .Linvalid_dynamic
    cmp qword ptr [rip + .Lloaded_dynamic_count], 0
    je .Linvalid_dynamic

    mov edi, 0x40b000
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
    mov rdi, qword ptr [rip + .Lloaded_dynamic_address]
    mov rcx, qword ptr [rip + .Lloaded_dynamic_count]
    pop r15
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
    cmp rax, 0x40b000
    jne .Linvalid_dynamic
    cmp dword ptr [0x40b000], 0x464c457f
    jne .Linvalid_dynamic
    cmp byte ptr [0x40c100], 0x48
    jne .Linvalid_dynamic

    mov edi, 0x40b000
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
.Lhostname_path:
    .asciz "/etc/hostname"

.section .data,"aw",@progbits
    .ascii "DYNAMIC!"

.section .bss.loader,"aw",@nobits
.balign 8
.Lmain_phdr:
    .zero 8
.Linitial_stack:
    .zero 8
.Lmain_phnum:
    .zero 8
.Lmain_entry:
    .zero 8
.Ldependency_base:
    .zero 8
.Lauxiliary_dependency_base:
    .zero 8
.Lsecondary_needed:
    .zero 8
.Lmain_relocation_size:
    .zero 8
.Lmain_relocation_entry_size:
    .zero 8
.Lmain_relative_count:
    .zero 8
.Lmain_jump_relocation_size:
    .zero 8
.Lmain_jump_relocation_kind:
    .zero 8
.Lmain_preinit_array:
    .zero 8
.Lmain_preinit_array_size:
    .zero 8
.Lmain_init_function:
    .zero 8
.Lmain_init_array:
    .zero 8
.Lmain_init_array_size:
    .zero 8
.Lauxiliary_name:
    .zero 8
.Lprimary_dynamic_address:
    .zero 8
.Lprimary_dynamic_count:
    .zero 8
.Lauxiliary_symbol_table:
    .zero 8
.Lauxiliary_hash_table:
    .zero 8
.Lauxiliary_string_table:
    .zero 8
.Lprimary_init_array:
    .zero 8
.Lprimary_init_function:
    .zero 8
.Lprimary_init_array_size:
    .zero 8
.Lprimary_relocation_size:
    .zero 8
.Lprimary_relocation_entry_size:
    .zero 8
.Lprimary_relative_count:
    .zero 8
.Lprimary_jump_relocation_size:
    .zero 8
.Lprimary_jump_relocation_kind:
    .zero 8
.Lprimary_flags:
    .zero 8
.Lauxiliary_init_array:
    .zero 8
.Lauxiliary_init_function:
    .zero 8
.Lauxiliary_init_array_size:
    .zero 8
.Lauxiliary_relocation_size:
    .zero 8
.Lauxiliary_relocation_entry_size:
    .zero 8
.Lauxiliary_relative_count:
    .zero 8
.Lauxiliary_jump_relocation_size:
    .zero 8
.Lauxiliary_jump_relocation_kind:
    .zero 8
.Lauxiliary_flags:
    .zero 8
.Lexpected_phnum:
    .zero 8
.Lloaded_dynamic_address:
    .zero 8
.Lloaded_dynamic_count:
    .zero 8

.section .data.rel.ro,"aw",@progbits
    .quad ginux_linux_interpreter
#endif

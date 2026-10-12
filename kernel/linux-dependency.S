.intel_syntax noprefix
.section .text
.global ginux_dependency_symbol
.type ginux_dependency_symbol,@function
.Lginux_dependency_local:
ginux_dependency_symbol:
#ifdef GINUX_LINKED
    cmp qword ptr [rip + .Ldependency_initialized], 0x4758
    jne .Ldependency_invalid
#endif
#ifndef GINUX_LINKED
    mov edx, 0x610100
    cmp qword ptr [0x6131c0], rdx
    jne .Ldependency_invalid
    cmp qword ptr [0x6131c8], rdx
    jne .Ldependency_invalid
    cmp qword ptr [0x6131d0], rdx
    jne .Ldependency_invalid
#endif
    movabs rax, 0x59434e45444e4947
    ret

.Ldependency_invalid:
    xor eax, eax
    ret

#ifdef GINUX_LINKED
.global ginux_dependency_init
.hidden ginux_dependency_init
.type ginux_dependency_init,@function
ginux_dependency_init:
    cmp qword ptr [0x405008], 1
    jne .Ldependency_init_done
    mov rdx, qword ptr [rip + ginux_dependency_tls_template@GOTTPOFF]
    movabs rax, 0x0a21534c544f5344
    cmp qword ptr fs:[rdx], rax
    jne .Ldependency_init_done
    cmp qword ptr fs:[rdx + 8], 0
    jne .Ldependency_init_done
    mov qword ptr [rip + .Ldependency_initialized], 0x47
.Ldependency_init_done:
    ret

.Ldependency_anchor:
    mov rax, qword ptr [rip + ginux_dependency_symbol@GOTPCREL]
    call ginux_dependency_symbol@PLT
    ret

.Ldependency_initialize:
    cmp qword ptr [rip + .Ldependency_initialized], 0x47
    jne .Ldependency_initialize_done
    cmp edi, 1
    je .Ldependency_initialize_single_vector
    cmp edi, 2
    jne .Ldependency_initialize_done
    cmp qword ptr [rsi + 16], 0
    jne .Ldependency_initialize_done
    cmp qword ptr [rdx + 16], 0
    jne .Ldependency_initialize_done
    mov rax, qword ptr [rsi + 8]
    movabs rcx, 0x2d63696d616e7964
    cmp qword ptr [rax], rcx
    jne .Ldependency_initialize_done
    cmp dword ptr [rax + 8], 0x63657865
    jne .Ldependency_initialize_done
    cmp byte ptr [rax + 12], 0
    jne .Ldependency_initialize_done
    mov rax, qword ptr [rdx]
    movabs rcx, 0x00313d58554e4947
    cmp qword ptr [rax], rcx
    jne .Ldependency_initialize_done
    mov rax, qword ptr [rdx + 8]
    movabs rcx, 0x4f4d5f58554e4947
    cmp qword ptr [rax], rcx
    jne .Ldependency_initialize_done
    movabs rcx, 0x00636578653d4544
    cmp qword ptr [rax + 8], rcx
    jne .Ldependency_initialize_done
    jmp .Ldependency_initialize_common
.Ldependency_initialize_single_vector:
    cmp qword ptr [rsi + 8], 0
    jne .Ldependency_initialize_done
    cmp qword ptr [rdx + 8], 0
    jne .Ldependency_initialize_done
.Ldependency_initialize_common:
    mov rax, qword ptr [rsi]
    cmp dword ptr [rax], 0x696e692f
    jne .Ldependency_initialize_done
    cmp word ptr [rax + 4], 0x74
    jne .Ldependency_initialize_done
    mov rax, qword ptr [rdx]
    movabs rcx, 0x00313d58554e4947
    cmp qword ptr [rax], rcx
    jne .Ldependency_initialize_done
    mov qword ptr [rip + .Ldependency_initialized], 0x4758
.Ldependency_initialize_done:
    ret

.section .text.secondary,"ax",@progbits
.Ldependency_secondary:
    mov eax, 0x47584e32
    ret

.section .rodata,"a",@progbits
.Ldependency_read_only:
    .quad 0x594c4e4f44414552

.section .init_array,"aw",@init_array
    .quad .Ldependency_initialize

.section .data,"aw",@progbits
.Ldependency_initialized:
    .quad 0

.section .tdata,"awT",@progbits
.balign 8
.global ginux_dependency_tls_template
.hidden ginux_dependency_tls_template
ginux_dependency_tls_template:
    .ascii "DSOTLS!\n"

.section .tbss,"awT",@nobits
.balign 8
.global ginux_dependency_tls_zero
.hidden ginux_dependency_tls_zero
ginux_dependency_tls_zero:
    .space 8
#endif

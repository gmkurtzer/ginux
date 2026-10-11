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
    mov qword ptr [rip + .Ldependency_initialized], 0x47
    ret

.Ldependency_anchor:
    mov rax, qword ptr [rip + ginux_dependency_symbol@GOTPCREL]
    call ginux_dependency_symbol@PLT
    ret

.Ldependency_initialize:
    cmp qword ptr [rip + .Ldependency_initialized], 0x47
    jne .Ldependency_initialize_done
    cmp edi, 1
    jne .Ldependency_initialize_done
    cmp qword ptr [rsi + 8], 0
    jne .Ldependency_initialize_done
    cmp qword ptr [rdx + 8], 0
    jne .Ldependency_initialize_done
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
#endif

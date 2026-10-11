.intel_syntax noprefix
.section .text
.global ginux_auxiliary_symbol
.type ginux_auxiliary_symbol,@function
.Lauxiliary_local:
ginux_auxiliary_symbol:
    cmp qword ptr [rip + .Lauxiliary_initialized], 0x4758
    jne .Lauxiliary_uninitialized
    movabs rax, 0x5952414958554147
    ret
.Lauxiliary_uninitialized:
    xor eax, eax
    ret

.Lauxiliary_initialize:
    cmp edi, 1
    jne .Lauxiliary_initialize_done
    cmp qword ptr [rsi + 8], 0
    jne .Lauxiliary_initialize_done
    cmp qword ptr [rdx + 8], 0
    jne .Lauxiliary_initialize_done
    mov rax, qword ptr [rsi]
    cmp dword ptr [rax], 0x696e692f
    jne .Lauxiliary_initialize_done
    cmp word ptr [rax + 4], 0x74
    jne .Lauxiliary_initialize_done
    mov rax, qword ptr [rdx]
    movabs rcx, 0x00313d58554e4947
    cmp qword ptr [rax], rcx
    jne .Lauxiliary_initialize_done
    mov qword ptr [rip + .Lauxiliary_initialized], 0x4758
.Lauxiliary_initialize_done:
    ret

.Lauxiliary_anchor:
    mov rax, qword ptr [rip + ginux_auxiliary_symbol@GOTPCREL]
    call ginux_auxiliary_symbol@PLT
    ret

.section .init_array,"aw",@init_array
    .quad .Lauxiliary_initialize

.section .data,"aw",@progbits
.Lauxiliary_initialized:
    .quad 0

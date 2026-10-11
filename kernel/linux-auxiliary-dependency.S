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
    mov qword ptr [rip + .Lauxiliary_initialized], 0x4758
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

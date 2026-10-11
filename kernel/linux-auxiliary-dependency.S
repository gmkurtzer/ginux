.intel_syntax noprefix
.section .text
.global ginux_auxiliary_symbol
.type ginux_auxiliary_symbol,@function
.Lauxiliary_local:
ginux_auxiliary_symbol:
    movabs rax, 0x5952414958554147
    ret

.Lauxiliary_anchor:
    mov rax, qword ptr [rip + ginux_auxiliary_symbol@GOTPCREL]
    call ginux_auxiliary_symbol@PLT
    ret

.section .data.rel.ro,"aw",@progbits
    .quad .Lauxiliary_local

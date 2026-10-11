.intel_syntax noprefix
.section .text
.global ginux_dependency_symbol
ginux_dependency_symbol:
    mov edx, 0x610100
    cmp qword ptr [0x6131c0], rdx
    jne .Ldependency_invalid
    cmp qword ptr [0x6131c8], rdx
    jne .Ldependency_invalid
    cmp qword ptr [0x6131d0], rdx
    jne .Ldependency_invalid
    movabs rax, 0x59434e45444e4947
    ret

.Ldependency_invalid:
    xor eax, eax
    ret

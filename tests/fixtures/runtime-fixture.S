.section .text
.global _start
.type _start,@function
_start:
    mov 16(%rsp), %rsi
    mov $1, %rax
    mov $1, %rdi
    mov $11, %rdx
    syscall

    mov 32(%rsp), %rsi
    mov $1, %rax
    mov $1, %rdi
    mov $12, %rdx
    syscall

    mov $999, %rax
    syscall
    cmp $-38, %rax
    jne .Lfail

    mov $39, %rax
    syscall
    cmp $4242, %rax
    jne .Lfail

    mov $60, %rax
    xor %rdi, %rdi
    syscall

.Lfail:
    mov $60, %rax
    mov $1, %rdi
    syscall
.size _start, .-_start

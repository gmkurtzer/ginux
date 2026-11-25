.global _start
.type _start,@function
.section .text
_start:
    mov $39, %rax
    syscall
    cmp $4242, %rax
    jne fail

    mov $1, %rax
    mov $1, %rdi
    lea bypass_marker(%rip), %rsi
    mov $bypass_marker_len, %rdx
    syscall
    cmp $bypass_marker_len, %rax
    jne fail

    mov $63, %rax
    lea uname_buffer(%rip), %rdi
    syscall
    cmp $-1, %rax
    jne fail

    mov $60, %rax
    xor %rdi, %rdi
    syscall
    ud2

fail:
    mov $60, %rax
    mov $1, %rdi
    syscall
    ud2

.section .rodata
bypass_marker:
    .ascii "UNMEDIATED_PAYLOAD_WRITE\n"
.set bypass_marker_len, . - bypass_marker

.section .bss
.align 8
uname_buffer:
    .skip 390


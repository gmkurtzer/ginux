.global _start
_start:
 mov $1,%rax
 mov $1,%rdi
 lea text(%rip),%rsi
 mov $13,%rdx
 syscall
 mov $60,%rax
 xor %rdi,%rdi
 syscall
text: .ascii "exec-success\n"

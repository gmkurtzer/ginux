/* SPDX-License-Identifier: GPL-2.0-only */

	.section .text
	.global _start
	.type _start, @function
_start:
	mov $258, %rax		/* __NR_mkdirat */
	mov $-100, %rdi		/* AT_FDCWD */
	lea path(%rip), %rsi
	mov $0700, %rdx
	syscall
	cmp $-1, %rax		/* -EPERM */
	jne failure
	xor %rdi, %rdi
	jmp exit
failure:
	mov $1, %rdi
exit:
	mov $60, %rax		/* __NR_exit */
	syscall
	.size _start, .-_start

	.section .rodata
path:
	.asciz "/tmp/ginux-static-raw"

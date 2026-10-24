.section .text,"ax",@progbits
.p2align 4
.globl __vdso_clock_gettime
.type __vdso_clock_gettime,@function
__vdso_clock_gettime:
    testq %rsi, %rsi
    je .Lfallback

    # Fast-path the wall and monotonic clocks used by glibc and sysbench.
    # CPU-time clocks retain the syscall fallback because their accounting is
    # process specific rather than derivable from the invariant TSC.
    cmpl $0, %edi
    je .Lrealtime
    cmpl $5, %edi
    je .Lrealtime_coarse
    cmpl $8, %edi
    je .Lrealtime
    cmpl $11, %edi
    je .Lrealtime
    cmpl $1, %edi
    je .Lmonotonic
    cmpl $4, %edi
    je .Lmonotonic
    cmpl $6, %edi
    je .Lmonotonic_coarse
    cmpl $7, %edi
    je .Lmonotonic
    cmpl $9, %edi
    jne .Lfallback

.Lmonotonic:
    xorl %r8d, %r8d
    jmp .Lread_clock
.Lmonotonic_coarse:
    movl $1, %r8d
    jmp .Lread_clock
.Lrealtime:
    movl $2, %r8d
    jmp .Lread_clock
.Lrealtime_coarse:
    movl $3, %r8d

.Lread_clock:
    # AT_SYSINFO_EHDR is page aligned and this image occupies one page. The
    # adjacent lower, read-only page is Ginux's vvar time snapshot.
    leaq .Lread_clock(%rip), %r9
    andq $-4096, %r9
    subq $4096, %r9
    movq 8(%r9), %rcx
    testq %rcx, %rcx
    je .Lfallback
    rdtsc
    shlq $32, %rdx
    orq %rdx, %rax
    testb $2, %r8b
    jnz .Lwall_epoch
    subq 16(%r9), %rax
    xorl %r10d, %r10d
    jmp .Lscale
.Lwall_epoch:
    subq 32(%r9), %rax
    movq 24(%r9), %r10
.Lscale:
    xorl %edx, %edx
    divq %rcx
    addq %r10, %rax
    movq %rax, (%rsi)
    movq %rdx, %rax
    movl $1000000000, %r10d
    mulq %r10
    divq %rcx
    testb $1, %r8b
    jz .Lstore_nsec
    # Linux coarse clocks advance in 10 ms ticks for AT_CLKTCK=100.
    xorl %edx, %edx
    movl $10000000, %r10d
    divq %r10
    imulq %r10, %rax
.Lstore_nsec:
    movq %rax, 8(%rsi)
    xorl %eax, %eax
    ret

.Lfallback:
    movl $228, %eax
    syscall
    ret
.size __vdso_clock_gettime, .-__vdso_clock_gettime

.globl clock_gettime
.set clock_gettime, __vdso_clock_gettime

.section .note.GNU-stack,"",@progbits

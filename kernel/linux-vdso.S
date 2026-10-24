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
    cmpl $1, %edi
    je .Lmonotonic_fast
    cmpl $0, %edi
    je .Lrealtime
    cmpl $5, %edi
    je .Lrealtime_coarse
    cmpl $8, %edi
    je .Lrealtime
    cmpl $11, %edi
    je .Lrealtime
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
    jmp .Lread_clock

.Lmonotonic_fast:
    # sysbench and most elapsed-time measurements use CLOCK_MONOTONIC. QEMU's
    # invariant 1 GHz TSC maps ticks directly to nanoseconds, so keep that
    # overwhelmingly common case free of generic clock classification and
    # fixed-point multiplication. Other TSC ratios use the generic path.
    leaq .Lmonotonic_fast(%rip), %r9
    andq $-4096, %r9
    subq $4096, %r9
.Lmonotonic_fast_retry:
    movq 8(%r9), %rcx
    movabsq $4294967296, %rdx
    cmpq %rdx, %rcx
    jne .Lmonotonic
    rdtsc
    shlq $32, %rdx
    orq %rdx, %rax
    subq 16(%r9), %rax
    movq 24(%r9), %r10
    movq 32(%r9), %rdi
    addq %rax, %rdi
    movq %rdi, %rax
    shrq $9, %rax
    movabsq $19342813113834067, %rdx
    mulq %rdx
    shrq $11, %rdx
    addq %rdx, %r10
    imulq $1000000000, %rdx, %rax
    subq %rax, %rdi
    movq %r10, (%rsi)
    movq %rdi, 8(%rsi)
    xorl %eax, %eax
    ret

.Lread_clock:
    # AT_SYSINFO_EHDR is page aligned and this image occupies one page. The
    # adjacent lower, read-only page is Ginux's vvar time snapshot.
    leaq .Lread_clock(%rip), %r9
    andq $-4096, %r9
    subq $4096, %r9
.Lretry:
    movq 8(%r9), %rcx
    testq %rcx, %rcx
    je .Lfallback
    rdtsc
    shlq $32, %rdx
    orq %rdx, %rax
    subq 16(%r9), %rax
    testb $2, %r8b
    jnz .Lwall_epoch
    movq 24(%r9), %r10
    movq 32(%r9), %rdi
    jmp .Lscale
.Lwall_epoch:
    movq 40(%r9), %r10
    movq 48(%r9), %rdi
.Lscale:
    # Convert TSC ticks to nanoseconds with the Q32 multiplier prepared by
    # the kernel. Unlike the syscall-compatible prototype, the common path
    # contains no hardware division.
    movabsq $4294967296, %rdx
    cmpq %rdx, %rcx
    je .Lnanoseconds_ready
    mulq %rcx
    shrdq $32, %rdx, %rax
.Lnanoseconds_ready:
    addq %rax, %rdi
    movq %rdi, %rax
    shrq $9, %rax
    movabsq $19342813113834067, %rdx
    mulq %rdx
    shrq $11, %rdx
    addq %rdx, %r10
    imulq $1000000000, %rdx, %rax
    subq %rax, %rdi
    movq %rdi, %rax
    testb $1, %r8b
    jz .Lstore_nsec
    # Linux coarse clocks advance in 10 ms ticks for AT_CLKTCK=100.
    xorl %edx, %edx
    movl $10000000, %ecx
    divq %rcx
    imulq %rcx, %rax
.Lstore_nsec:
    movq %r10, (%rsi)
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

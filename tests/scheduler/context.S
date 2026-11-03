.intel_syntax noprefix
.text
.global check_context
.type check_context,@function
check_context:
push rbx
push rbp
push r12
push r13
push r14
push r15
sub rsp, 256
mov [rsp], rdi
mov [rsp + 8], rsi
mov rax, fs:0
mov [rsp + 16], rax
mov [rsp + 24], rsp
mov qword ptr [rsp - 128], 0x468a
mov qword ptr [rsp - 64], 0x7531
lea rax, [rsi + 1]
mov [rsp + 64], rax
lea rax, [rsi + 2]
mov [rsp + 72], rax
lea rax, [rsi + 3]
mov [rsp + 80], rax
lea rax, [rsi + 4]
mov [rsp + 88], rax
lea rax, [rsi + 5]
mov [rsp + 96], rax
lea rax, [rsi + 6]
mov [rsp + 104], rax
lea rax, [rsi + 7]
mov [rsp + 112], rax
lea rax, [rsi + 8]
mov [rsp + 120], rax
lea rax, [rsi + 9]
mov [rsp + 128], rax
lea rax, [rsi + 10]
mov [rsp + 136], rax
lea rax, [rsi + 11]
mov [rsp + 144], rax
lea rax, [rsi + 12]
mov [rsp + 152], rax
lea rax, [rsi + 13]
mov [rsp + 160], rax
lea rax, [rsi + 14]
mov [rsp + 168], rax
lea rax, [rsi + 15]
mov [rsp + 176], rax
vbroadcastsd ymm0, [rsp + 8]
vmovdqa ymm7, ymm0
vmovdqa ymm15, ymm0
mov dword ptr [rsp + 40], 0x3f80
ldmxcsr [rsp + 40]
mov word ptr [rsp + 44], 0x077f
fldcw [rsp + 44]
fld1
mov rax, [rsp + 64]
mov rbx, [rsp + 72]
mov rcx, [rsp + 80]
mov rdx, [rsp + 88]
mov rsi, [rsp + 96]
mov rdi, [rsp + 104]
mov rbp, [rsp + 112]
mov r8, [rsp + 120]
mov r9, [rsp + 128]
mov r10, [rsp + 136]
mov r11, [rsp + 144]
mov r12, [rsp + 152]
mov r13, [rsp + 160]
mov r14, [rsp + 168]
mov r15, [rsp + 176]
.Lcheck:
std
stc
pause
pushfq
pop qword ptr [rsp + 32]
cld
and qword ptr [rsp + 32], 0x401
cmp qword ptr [rsp + 32], 0x401
jne .Lflags
cmp rax, [rsp + 64]
jne .Lgpr
cmp rbx, [rsp + 72]
jne .Lgpr
cmp rcx, [rsp + 80]
jne .Lgpr
cmp rdx, [rsp + 88]
jne .Lgpr
cmp rsi, [rsp + 96]
jne .Lgpr
cmp rdi, [rsp + 104]
jne .Lgpr
cmp rbp, [rsp + 112]
jne .Lgpr
cmp r8, [rsp + 120]
jne .Lgpr
cmp r9, [rsp + 128]
jne .Lgpr
cmp r10, [rsp + 136]
jne .Lgpr
cmp r11, [rsp + 144]
jne .Lgpr
cmp r12, [rsp + 152]
jne .Lgpr
cmp r13, [rsp + 160]
jne .Lgpr
cmp r14, [rsp + 168]
jne .Lgpr
cmp r15, [rsp + 176]
jne .Lgpr
cmp rsp, [rsp + 24]
jne .Lstack
cmp qword ptr [rsp - 128], 0x468a
jne .Lstack
cmp qword ptr [rsp - 64], 0x7531
jne .Lstack
mov rax, fs:0
cmp rax, [rsp + 16]
jne .Ltls
vmovdqu [rsp + 192], ymm0
mov rax, [rsp + 8]
cmp rax, [rsp + 192]
jne .Lvector
cmp rax, [rsp + 200]
jne .Lvector
cmp rax, [rsp + 208]
jne .Lvector
cmp rax, [rsp + 216]
jne .Lvector
vmovdqu [rsp + 192], ymm7
mov rax, [rsp + 8]
cmp rax, [rsp + 192]
jne .Lvector
cmp rax, [rsp + 200]
jne .Lvector
cmp rax, [rsp + 208]
jne .Lvector
cmp rax, [rsp + 216]
jne .Lvector
vmovdqu [rsp + 192], ymm15
mov rax, [rsp + 8]
cmp rax, [rsp + 192]
jne .Lvector
cmp rax, [rsp + 200]
jne .Lvector
cmp rax, [rsp + 208]
jne .Lvector
cmp rax, [rsp + 216]
jne .Lvector
fst qword ptr [rsp + 224]
mov rax, 0x3ff0000000000000
cmp rax, [rsp + 224]
jne .Lx87
stmxcsr [rsp + 232]
cmp dword ptr [rsp + 232], 0x3f80
jne .Lvector
fnstcw [rsp + 236]
cmp word ptr [rsp + 236], 0x077f
jne .Lx87
mov rax, [rsp + 64]
dec qword ptr [rsp]
jnz .Lcheck
xor eax, eax
jmp .Lout
.Lgpr:
mov eax, 1
jmp .Lout
.Lflags:
mov eax, 2
jmp .Lout
.Lstack:
mov eax, 3
jmp .Lout
.Ltls:
mov eax, 4
jmp .Lout
.Lvector:
mov eax, 5
jmp .Lout
.Lx87:
mov eax, 6
jmp .Lout
.Lout:
cld
fninit
vzeroall
mov dword ptr [rsp + 40], 0x1f80
ldmxcsr [rsp + 40]
add rsp, 256
pop r15
pop r14
pop r13
pop r12
pop rbp
pop rbx
ret
.size check_context, .-check_context
.section .note.GNU-stack,"",@progbits

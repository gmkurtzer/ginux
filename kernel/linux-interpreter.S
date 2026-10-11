.intel_syntax noprefix
.section .text
.global ginux_linux_interpreter
ginux_linux_interpreter:
    lea rbx, [rip]
    and rbx, -4096

    mov rax, qword ptr [rbx + 0x3010]
    add rax, rbx
    mov rcx, qword ptr [rax]
    add rcx, rbx
    mov rdx, qword ptr [rax + 16]
    add rdx, rbx
    mov qword ptr [rcx], rdx

    mov rax, qword ptr [0x404010]
    mov rcx, qword ptr [rax]
    mov rdx, qword ptr [rax + 16]
    mov qword ptr [rcx], rdx

    mov eax, 0x400100
    jmp rax

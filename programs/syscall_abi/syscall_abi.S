.intel_syntax noprefix

# Checks
#
# - that the stack pointer is 16-byte aligned at the entry point, as the
#   System V ABI specifies (bit 15 of the exit code), and
# - the syscall ABI: every register except rax must survive `int 0x80`
#   (bit 0 = rbx, 1 = rcx, 2 = rdx, 3 = rsi, 4 = rdi, 5 = rbp, 6 = r8,
#   7 = r9, 8 = r10, 9 = r11, 10 = r12, 11 = r13, 12 = r14, 13 = r15).
#
# Exits with the bits of what was wrong (0 on success).

.global _start
_start:
        xor eax, eax
        test rsp, 0xF
        jz 1f
        or eax, 0x8000
1:      push rax # the alignment check's result, also checked after the call

        mov rbx, 0x1111111111111111
        mov rcx, 0x2222222222222222
        mov rdx, 0x3333333333333333
        mov rsi, 0x4444444444444444
        mov rdi, 0x5555555555555555
        mov rbp, 0x6666666666666666
        mov r8, 0x7777777777777777
        mov r9, 0x8888888888888888
        mov r10, 0x9999999999999999
        mov r11, 0xAAAAAAAAAAAAAAAA
        mov r12, 0xBBBBBBBBBBBBBBBB
        mov r13, 0xCCCCCCCCCCCCCCCC
        mov r14, 0xDDDDDDDDDDDDDDDD
        mov r15, 0xEEEEEEEEEEEEEEEE
        mov eax, 0x14 # getpid
        int 0x80

        # rax holds the system call's result, so it's free to use. The
        # result bits are collected in the stack slot pushed above.
        .macro check reg, value, bit
        movabs rax, \value
        cmp \reg, rax
        je 2f
        or qword ptr [rsp], \bit
2:
        .endm
        check rbx, 0x1111111111111111, 0x1
        check rcx, 0x2222222222222222, 0x2
        check rdx, 0x3333333333333333, 0x4
        check rsi, 0x4444444444444444, 0x8
        check rdi, 0x5555555555555555, 0x10
        check rbp, 0x6666666666666666, 0x20
        check r8, 0x7777777777777777, 0x40
        check r9, 0x8888888888888888, 0x80
        check r10, 0x9999999999999999, 0x100
        check r11, 0xAAAAAAAAAAAAAAAA, 0x200
        check r12, 0xBBBBBBBBBBBBBBBB, 0x400
        check r13, 0xCCCCCCCCCCCCCCCC, 0x800
        check r14, 0xDDDDDDDDDDDDDDDD, 0x1000
        check r15, 0xEEEEEEEEEEEEEEEE, 0x2000
        pop rax

        mov edi, eax
        mov eax, 0x1 # exit
        int 0x80

        # Hang if we make it back.
loop:
        jmp loop

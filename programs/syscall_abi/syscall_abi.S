.intel_syntax noprefix

# Checks the syscall ABI: every register except eax must survive `int 0x80`.
# Exits with a bitmask of the registers that were clobbered (0 on success):
# bit 0 = ebx, 1 = ecx, 2 = edx, 3 = esi, 4 = edi, 5 = ebp.

.global _start
_start:
        mov ebx, 0x11111111
        mov ecx, 0x22222222
        mov edx, 0x33333333
        mov esi, 0x44444444
        mov edi, 0x55555555
        mov ebp, 0x66666666
        mov eax, 0x14 # getpid
        int 0x80

        xor eax, eax
        cmp ebx, 0x11111111
        je 1f
        or eax, 0x1
1:      cmp ecx, 0x22222222
        je 1f
        or eax, 0x2
1:      cmp edx, 0x33333333
        je 1f
        or eax, 0x4
1:      cmp esi, 0x44444444
        je 1f
        or eax, 0x8
1:      cmp edi, 0x55555555
        je 1f
        or eax, 0x10
1:      cmp ebp, 0x66666666
        je 1f
        or eax, 0x20
1:
        mov ebx, eax
        mov eax, 0x1 # exit
        int 0x80

        # Hang if we make it back.
loop:
        jmp loop

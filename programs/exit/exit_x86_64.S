.intel_syntax noprefix

# The x86_64 version of exit.S.
.global _start
_start:
        # Execute the bochs break instruction so we know we made it here...
        xchg bx, bx
        # ...then immediately execute KidneyOS's exit syscall, passing an exit
        # code of 0 to indicate success.
        mov eax, 0x1
        xor edi, edi
        int 0x80

        # Hang if we make it back.
loop:
        jmp loop

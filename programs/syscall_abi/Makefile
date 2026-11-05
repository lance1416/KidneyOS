AS = i686-unknown-linux-gnu-as
LD = i686-unknown-linux-gnu-ld

syscall_abi: syscall_abi.o
	$(LD) -o $@ $^

syscall_abi.o: syscall_abi.S
	$(AS) -o $@ $<

.PHONY: clean
clean:
	rm -f syscall_abi syscall_abi.o

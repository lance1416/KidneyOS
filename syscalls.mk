PROJECT_DIR := $(dir $(realpath $(lastword $(MAKEFILE_LIST))))

# Stubs are explicitly debug so they don't get inlined.
SYSCALL_LIB := $(PROJECT_DIR)build/syscalls/libkidneyos_syscalls.rlib

# Cargo no longer copies ("uplifts") rlibs out of its build directory, so the
# rlib's path is taken from cargo's JSON artifact messages and the file is
# copied to a fixed location for the C programs to link against.
$(SYSCALL_LIB): $(PROJECT_DIR)syscalls/src/lib.rs $(PROJECT_DIR)syscalls/src/defs.rs
	mkdir -p $(dir $@)
	cd $(PROJECT_DIR)syscalls && cargo build --message-format=json-render-diagnostics > $@.json
	cp "$$(grep -o '"[^"]*/libkidneyos_syscalls-[^"]*\.rlib"' $@.json | head -n 1 | tr -d '"')" $@
	rm -f $@.json

clean-syscall:
	# This might clean some other OS files too.
	rm -rf $(PROJECT_DIR)target

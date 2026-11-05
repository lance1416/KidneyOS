# Builds the syscall stub library that the C user programs link against.
# Included by programs.mk, which sets PROJECT_DIR (no trailing slash).

SYSCALL_LIB := $(PROJECT_DIR)/build/syscalls/libkidneyos_syscalls.rlib
SYSCALL_INCLUDE := $(PROJECT_DIR)/syscalls/include
SYSCALL_SOURCES := $(addprefix $(PROJECT_DIR)/syscalls/,Cargo.toml build.rs bindgen.toml) \
	$(wildcard $(PROJECT_DIR)/syscalls/src/*.rs)

# Cargo no longer copies ("uplifts") rlibs out of its build directory, so the
# rlib's path is taken from cargo's JSON artifact messages and the file is
# copied to a fixed location for the C programs to link against. Stubs are
# built in debug mode so they don't get inlined.
$(SYSCALL_LIB): $(SYSCALL_SOURCES)
	mkdir -p $(dir $@)
	cd $(PROJECT_DIR)/syscalls && cargo build --message-format=json-render-diagnostics > $@.json
	cp "$$(grep -o '"[^"]*/libkidneyos_syscalls-[^"]*\.rlib"' $@.json | head -n 1 | tr -d '"')" $@
	rm -f $@.json

.PHONY: clean-syscall
clean-syscall: ## Remove the syscall stub library built for the C programs
	rm -rf $(dir $(SYSCALL_LIB))

# User programs (programs/*). Included by the Makefile, but also usable on its
# own from the repository root: `make -f programs.mk`.

PROJECT_DIR ?= $(patsubst %/,%,$(dir $(realpath $(lastword $(MAKEFILE_LIST)))))
CROSS_PREFIX ?= i686-unknown-linux-gnu-
ARCH ?= i686
# Prefix of the x86_64 binutils and gcc: none on x86_64 hosts, where they're
# the native ones.
X86_64_CROSS_PREFIX ?= $(if $(filter x86_64,$(shell uname -m)),,x86_64-unknown-linux-gnu-)

# The programs are built for the architecture of the kernel (ARCH).
ifeq ($(ARCH),x86_64)
# Rust programs use the standard target (with SSE, which the kernel saves).
PROGRAMS_TRIPLE := x86_64-unknown-linux-gnu
PROGRAMS_CROSS_PREFIX := $(X86_64_CROSS_PREFIX)
NATIVE_PROGRAMS_SUBDIR := bin-x86_64
ASM_SUFFIX := _x86_64
# Static, non-PIE executables like the Rust ones. The kernel starts programs
# with the stack 16-byte aligned (as the ABI specifies for the entry point),
# but the C programs' `_start` is a function, which expects a return address
# on the stack: realign it.
ARCH_C_FLAGS := -static -no-pie -mstackrealign
else
PROGRAMS_TRIPLE := i586-unknown-linux-gnu
PROGRAMS_CROSS_PREFIX := $(CROSS_PREFIX)
NATIVE_PROGRAMS_SUBDIR := bin
ASM_SUFFIX :=
ARCH_C_FLAGS :=
endif

# Only when used on its own; the Makefile sets its own default goal first.
ifeq ($(.DEFAULT_GOAL),)
.DEFAULT_GOAL := programs
endif

include $(PROJECT_DIR)/syscalls.mk

# $(call cargo-deps,FILE.d): the prerequisites listed in the cargo dep-info file
# FILE.d that still exist. Cargo's .d files keep listing source files that have
# since been deleted or moved, which would make make fail with "No rule to make
# target"; dropping them is safe, since removing a source file always changes
# another one that is still listed (the one with the `mod` declaration).
cargo-deps = $(wildcard $(filter-out %:,$(shell cat $(1) 2>/dev/null)))

# Rust programs: one cargo workspace (programs/Cargo.toml) built for
# PROGRAMS_TRIPLE. The kernel and execve include_bytes!() binaries from
# here, so this location deliberately ignores CARGO_TARGET_DIR.
PROGRAMS_TARGET_DIR := $(PROJECT_DIR)/build/programs
RUST_PROGRAMS_DIR := $(PROGRAMS_TARGET_DIR)/$(PROGRAMS_TRIPLE)/release
RUST_PROGRAMS := bss example_rust execve pipes waitpid
# Cargo reads programs/.cargo/config.toml only when it is run from programs/.
PROGRAMS_CARGO := cd $(PROJECT_DIR)/programs && CARGO_TARGET_DIR=$(PROGRAMS_TARGET_DIR) cargo
# What cargo build and clippy need to build the programs for PROGRAMS_TRIPLE.
PROGRAMS_CARGO_ARGS := --target $(PROGRAMS_TRIPLE)
PROGRAMS_CARGO_INPUTS := $(addprefix $(PROJECT_DIR)/programs/,Cargo.toml Cargo.lock .cargo/config.toml) \
	$(SYSCALL_SOURCES)

# C and assembly programs, built without cargo.
NATIVE_PROGRAMS_DIR := $(PROGRAMS_TARGET_DIR)/$(NATIVE_PROGRAMS_SUBDIR)
C_PROGRAMS := example_c basic mmap
ASM_PROGRAMS := exit syscall_abi
C_FLAGS := -ffreestanding -fno-stack-protector -nostdlib -nostartfiles -e _start \
	-I $(SYSCALL_INCLUDE) $(ARCH_C_FLAGS)

PROGRAMS := $(addprefix $(RUST_PROGRAMS_DIR)/,$(RUST_PROGRAMS)) \
	$(addprefix $(NATIVE_PROGRAMS_DIR)/,$(C_PROGRAMS) $(ASM_PROGRAMS))

.PHONY: programs
programs: $(PROGRAMS) ## Build every user program into build/programs

# Each binary depends on its own manifest and sources; once it has been built,
# cargo's dep-info file also lists everything else it was built from (the
# syscalls crate, include_bytes!() inputs, ...).
define rust-program
$(RUST_PROGRAMS_DIR)/$(1): $(PROGRAMS_CARGO_INPUTS) $(PROJECT_DIR)/programs/$(1)/Cargo.toml \
		$(wildcard $(PROJECT_DIR)/programs/$(1)/build.rs $(PROJECT_DIR)/programs/$(1)/src/*.rs) \
		$(call cargo-deps,$(RUST_PROGRAMS_DIR)/$(1).d)
	$$(PROGRAMS_CARGO) build $$(PROGRAMS_CARGO_ARGS) --release --package $(1)
endef
$(foreach p,$(RUST_PROGRAMS),$(eval $(call rust-program,$(p))))

# execve embeds example_rust's binary, so it has to be built after it.
$(RUST_PROGRAMS_DIR)/execve: $(RUST_PROGRAMS_DIR)/example_rust

# $(call c-program,NAME,SOURCE)
define c-program
$(NATIVE_PROGRAMS_DIR)/$(1): $(PROJECT_DIR)/programs/$(2) $(SYSCALL_LIB)
	mkdir -p $$(dir $$@)
	$$(PROGRAMS_CROSS_PREFIX)gcc $$< -o $$@ $$(SYSCALL_LIB) $$(C_FLAGS)
endef
$(eval $(call c-program,example_c,example_c/main.c))
$(eval $(call c-program,basic,fs/basic.c))
$(eval $(call c-program,mmap,fs/mmap.c))

define asm-program
$(NATIVE_PROGRAMS_DIR)/$(1): $(PROJECT_DIR)/programs/$(1)/$(1)$(ASM_SUFFIX).S
	mkdir -p $$(dir $$@)
	$$(PROGRAMS_CROSS_PREFIX)as -o $$@.o $$<
	$$(PROGRAMS_CROSS_PREFIX)ld -o $$@ $$@.o
endef
$(foreach p,$(ASM_PROGRAMS),$(eval $(call asm-program,$(p))))

.PHONY: clean-programs
clean-programs: clean-syscall ## Remove all built user programs
	rm -rf $(PROGRAMS_TARGET_DIR)

.PHONY: clean
clean:: clean-programs
	@:

# Run `make help` for a list of targets.

PROJECT_DIR := $(patsubst %/,%,$(dir $(realpath $(lastword $(MAKEFILE_LIST)))))

.DEFAULT_GOAL := iso

-include local.mk

CARGO_TARGET_DIR ?= $(PROJECT_DIR)/build/target
PROFILE ?= dev
# Prefix of the x86_64 binutils and gcc that build the C and assembly user
# programs: none on x86_64 hosts, where they're the native ones. Override it in
# local.mk if yours are different (e.g. x86_64-elf- for Homebrew's on macOS).
CROSS_PREFIX ?= $(if $(filter x86_64,$(shell uname -m)),,x86_64-unknown-linux-gnu-)
# Override in local.mk if your grub-mkrescue has a different name (e.g. Homebrew's
# i686-elf-grub-mkrescue on macOS).
GRUB_MKRESCUE ?= grub-mkrescue

export CARGO_TARGET_DIR

# The user programs the kernel starts when booting (a comma-separated list),
# each as its own process; see kernel/build.rs.
KIDNEYOS_INIT ?= pipes
# Set to run a test while booting; see kmain.rs. With KIDNEYOS_TEST=pagefault,
# the kernel writes to an unmapped address, and the page fault handler maps it.
KIDNEYOS_TEST ?=

# What the smoke test waits for on the serial port (a list of quoted strings;
# see build-support/qemu-smoke.bash): the IDE driver having read the disk, the
# root file system, both preemption test threads (one of which only gets to run
# if the timer preempts the other), init (pipes, by default) making a system
# call (pipe) and exiting successfully, and the shell prompt.
ifeq ($(KIDNEYOS_TEST),)
SMOKE_EXPECT := 'IDE subsystem initialized' 'Root filesystem mounted' \
	'was preempted while spinning' 'started after thread' \
	'syscall number 0x2A' 'exited with code 0' 'kidney:/$$'
else ifeq ($(KIDNEYOS_TEST),pagefault)
SMOKE_EXPECT := 'page fault: cr2=0x00000BADBEEF0128' 'KIDNEYOS_TEST=pagefault: passed'
else
$(error Unknown KIDNEYOS_TEST: $(KIDNEYOS_TEST) (expected pagefault))
endif

KERNEL_TARGET := x86_64-unknown-none
QEMU := qemu-system-x86_64
# The llvm-objcopy of the llvm-tools component, which handles ELF64 on any host.
OBJCOPY = $(shell rustc --print sysroot)/lib/rustlib/$(shell rustc --print host-tuple)/bin/llvm-objcopy

ifeq ($(PROFILE),dev)
ARTIFACT_DIR := $(CARGO_TARGET_DIR)/$(KERNEL_TARGET)/debug
else ifeq ($(PROFILE),release)
ARTIFACT_DIR := $(CARGO_TARGET_DIR)/$(KERNEL_TARGET)/release
else
$(error Unhandled profile: $(PROFILE))
endif

RAW_KERNEL := $(ARTIFACT_DIR)/kidneyos
ISOFILES := build/isofiles
KERNEL_SYM := build/kernel.sym
ISO := build/kidneyos.iso
# The disk QEMU and Bochs attach. It is unpacked from the compressed copy in
# build-support/ on first use, and then kept, so that changes made to it by
# KidneyOS persist across runs. `make reset-disk` restores the original.
DISK := build/disk.img
DISK_GZ := build-support/disk.img.gz

include programs.mk

.PHONY: help
help: ## Show this help
	@echo 'Usage: make [target] [VARIABLE=value ...]'
	@echo
	@awk 'BEGIN { FS = ":.*## " } /^[a-zA-Z0-9_.-]+:.*## / { printf "  %-18s %s\n", $$1, $$2 }' \
	    $(MAKEFILE_LIST)
	@echo
	@echo 'Variables: PROFILE=dev|release, QEMU_DEBUG=<qemu -d items>,'
	@echo '           BOCHS_DISPLAY=<bochs display_library>, KIDNEYOS_INIT=<program>[,<program>...],'
	@echo '           KIDNEYOS_TEST=pagefault'

.PHONY: iso
iso: $(ISO) ## Build the bootable ISO (build/kidneyos.iso); the default target

# Rust Builds

# Records the settings that the kernel is built with (but cargo can't see
# changing), so that the kernel is rebuilt when they change. PROFILE is here
# because each profile's kernel is a different file: switching back to one
# that is already built must still put it in the ISO.
KERNEL_CONFIG := PROFILE=$(PROFILE) KIDNEYOS_TEST=$(KIDNEYOS_TEST) KIDNEYOS_INIT=$(KIDNEYOS_INIT)
KERNEL_CONFIG_STAMP := build/kernel-config

.PHONY: FORCE
$(KERNEL_CONFIG_STAMP): FORCE
	@mkdir -p $(dir $@)
	@echo '$(KERNEL_CONFIG)' | cmp -s - $@ || echo '$(KERNEL_CONFIG)' > $@

# The built-in target is already freestanding, soft-float, without a red zone and
# uses the kernel code model, and core and alloc come prebuilt for it; the
# rustflags in .cargo/config.toml link it at a fixed address rather than as a
# static PIE. The kernel embeds the init programs (see kernel/build.rs).
$(RAW_KERNEL): kernel/Cargo.toml Cargo.lock build-support/x86_64.ld $(PROGRAMS) \
		$(KERNEL_CONFIG_STAMP) $(call cargo-deps,$(ARTIFACT_DIR)/kidneyos.d)
	KIDNEYOS_TEST='$(KIDNEYOS_TEST)' KIDNEYOS_INIT='$(KIDNEYOS_INIT)' cargo rustc \
	  --bin kidneyos \
	  --manifest-path $< \
	  --profile $(PROFILE) \
	  --target $(KERNEL_TARGET) \
	  -- \
	  -C link-arg=-T -C link-arg=build-support/x86_64.ld \
	  -C link-arg=-z -C link-arg=max-page-size=0x1000 \
	  -C link-arg=-n
	@touch $@

# Kernel Post-Build

$(ISOFILES)/boot/kernel.bin: $(RAW_KERNEL)
	mkdir -p $(dir $@)
	$(OBJCOPY) --strip-debug $< $@

$(KERNEL_SYM): $(RAW_KERNEL)
	mkdir -p $(dir $@)
	$(OBJCOPY) --only-keep-debug $< $@

$(ISOFILES)/boot/grub/grub.cfg: build-support/grub.cfg
	mkdir -p $(dir $@)
	cp $< $@

$(ISO): $(ISOFILES)/boot/kernel.bin $(ISOFILES)/boot/grub/grub.cfg
	$(GRUB_MKRESCUE) -o $@ $(ISOFILES)

# Disk Image

$(DISK): $(DISK_GZ)
	mkdir -p $(dir $@)
	gzip -dc $< > $@.tmp
	mv $@.tmp $@

.PHONY: reset-disk
reset-disk: ## Discard changes to build/disk.img by unpacking it again
	rm -f $(DISK)
	$(MAKE) $(DISK)

# Running

# Set to a comma-separated list of QEMU log items to have QEMU print them, e.g.
# `make run-qemu QEMU_DEBUG=int,mmu,pcall,cpu_reset,guest_errors`. See
# `$(QEMU) -d help` for the full list.
QEMU_DEBUG ?=
QEMU_FLAGS := -no-reboot -no-shutdown -m 4G -cdrom $(ISO) \
	-drive format=raw,file=$(DISK),if=ide \
	-boot d \
	-cpu Haswell,+rdrand \
	$(if $(QEMU_DEBUG),-d $(QEMU_DEBUG))

.PHONY: run-qemu
run-qemu: $(ISO) $(DISK) ## Run KidneyOS in QEMU
	$(QEMU) $(QEMU_FLAGS)

.PHONY: run-qemu-gdb
run-qemu-gdb: $(ISO) $(DISK) $(KERNEL_SYM) ## Run in QEMU, paused until GDB attaches on :1234
	$(QEMU) -s -S $(QEMU_FLAGS)

.PHONY: run-qemu-ng
run-qemu-ng: $(ISO) $(DISK) ## Run in QEMU without a display (serial on the terminal)
	# NOTE: You can quit with Ctrl-A X
	$(QEMU) -nographic $(QEMU_FLAGS)

.PHONY: run-qemu-ng-gdb
run-qemu-ng-gdb: $(ISO) $(DISK) $(KERNEL_SYM) ## run-qemu-ng, paused until GDB attaches on :1234
	# NOTE: You can quit with Ctrl-A X
	$(QEMU) -nographic -s -S $(QEMU_FLAGS)

SMOKE_TIMEOUT ?= 120

SMOKE_TEST := build-support/qemu-smoke.bash $(QEMU) $(SMOKE_TIMEOUT) build/smoke-test.log \
	$(SMOKE_EXPECT) -- $(QEMU_FLAGS)

.PHONY: smoke-test
smoke-test: $(ISO) $(DISK) ## Boot in QEMU, and wait for init to succeed and the shell prompt
	$(SMOKE_TEST)

# The user program tests: each boots the kernel with init programs
# (PROGRAM_TEST_INIT_name) and waits for what shows that they work
# (PROGRAM_TEST_EXPECT_name). "N*string" means at least N lines with string.
PROGRAM_TESTS := bss execve waitpid syscall_abi
PROGRAM_TEST_INIT_bss := bss
PROGRAM_TEST_EXPECT_bss := 'exited with code 0'
# execve runs example_rust, which exits with 1.
PROGRAM_TEST_INIT_execve := execve
PROGRAM_TEST_EXPECT_execve := 'syscall number 0xB' 'exited with code 1'
# waitpid waits for example_rust (see programs/waitpid).
PROGRAM_TEST_INIT_waitpid := waitpid,example_rust
PROGRAM_TEST_EXPECT_waitpid := 'exited with code 1' 'exited with code 0'
PROGRAM_TEST_INIT_syscall_abi := syscall_abi
PROGRAM_TEST_EXPECT_syscall_abi := 'exited with code 0'
# Two processes using SSE at the same time (which the kernel must switch).
PROGRAM_TESTS += sse
PROGRAM_TEST_INIT_sse := sse,sse
PROGRAM_TEST_EXPECT_sse := '2*exited with code 0'

.PHONY: smoke-programs
smoke-programs: $(DISK) $(RUST_PROGRAMS_DIR)/sse ## Boot with each user program test as init, and check its result
	@# The sse test only tests something if the compiler used SSE.
	$(CROSS_PREFIX)objdump -d $(RUST_PROGRAMS_DIR)/sse | grep -q xmm
	$(foreach test,$(PROGRAM_TESTS),\
	  $(MAKE) --no-print-directory KIDNEYOS_INIT=$(PROGRAM_TEST_INIT_$(test)) iso && \
	  build-support/qemu-smoke.bash $(QEMU) $(SMOKE_TIMEOUT) build/smoke-$(test).log \
	    $(PROGRAM_TEST_EXPECT_$(test)) -- $(QEMU_FLAGS) && ) true
	@# Leave the default kernel built.
	$(MAKE) --no-print-directory iso

.PHONY: smoke
smoke: smoke-test ## Alias for smoke-test

# For an ISO that was built elsewhere (CI downloads it from the build job), so
# make doesn't try to rebuild it.
.PHONY: smoke-test-iso
smoke-test-iso: $(DISK) ## Like smoke-test, but boot the existing build/kidneyos.iso as is
	@test -f $(ISO) || { echo "$(ISO) does not exist; run 'make iso' first." >&2; exit 1; }
	$(SMOKE_TEST)

# Bochs' display library. The X11 debugger GUI is only available on Linux;
# elsewhere Bochs' default is used.
ifeq ($(shell uname -s),Linux)
BOCHS_DISPLAY ?= x, options="gui_debug"
else
BOCHS_DISPLAY ?=
endif

.PHONY: run-bochs
run-bochs: $(ISO) $(DISK) ## Run KidneyOS in Bochs (which must support x86-64)
	bochs -q -f bochsrc.txt $(if $(BOCHS_DISPLAY),'display_library: $(BOCHS_DISPLAY)')

# Docs

.PHONY: docs
docs: ## Build the book into build/docs
	mdbook build docs

.PHONY: docs-serve
docs-serve: ## Serve the book locally, rebuilding on changes
	mdbook serve docs

# Tests and Coverage

COVERAGE_DIR := $(PROJECT_DIR)/build/coverage
COVERAGE_STAMP := $(COVERAGE_DIR)/.tests-passed
TEST_BINARY_DIR := $(CARGO_TARGET_DIR)/x86_64-unknown-linux-gnu/debug

# The tests are ordinary Linux programs (see .cargo/config.toml). The syscall
# stubs have no tests, and excluding them keeps them out of the test binaries
# (where they would replace the C library's functions of the same names).
# The rustflags are passed with --config rather than RUSTFLAGS so that they are
# added to the ones in .cargo/config.toml instead of replacing them.
.PHONY: test
test: ## Run the tests (with coverage instrumentation)
	rm -rf $(COVERAGE_DIR)
	mkdir -p $(COVERAGE_DIR)
	LLVM_PROFILE_FILE='$(COVERAGE_DIR)/%p-%m.profraw' cargo test \
	  --target x86_64-unknown-linux-gnu --workspace --exclude kidneyos-syscalls \
	  --config 'target.x86_64-unknown-linux-gnu.rustflags = ["-C", "instrument-coverage"]'
	touch $(COVERAGE_STAMP)

$(COVERAGE_STAMP):
	$(MAKE) test

.PHONY: report-coverage
report-coverage: $(COVERAGE_STAMP) ## Write an HTML coverage report to build/coverage/index.html
	grcov $(COVERAGE_DIR) --binary-path $(TEST_BINARY_DIR) \
	    --branch --output-path $(COVERAGE_DIR) --output-types html --source-dir .

$(COVERAGE_DIR)/coverage.md: $(COVERAGE_STAMP)
	grcov $(COVERAGE_DIR) --binary-path $(TEST_BINARY_DIR) \
	    --branch --output-path $@ --output-types markdown --source-dir .

.PHONY: print-coverage
print-coverage: $(COVERAGE_DIR)/coverage.md ## Print the total test coverage
	tail -n 1 $<

.PHONY: coverage-summary
coverage-summary: $(COVERAGE_DIR)/coverage.md ## Print a Markdown coverage summary (use with make -s)
	@echo '### Test coverage'
	@echo
	@tail -n 1 $<

# Checks

# $(call need,TOOL): a recipe prefix that skips the rest of the line with a
# warning if TOOL isn't installed. In CI (where $CI is set), it fails instead.
need = if ! command -v $(1) >/dev/null 2>&1; then \
	  echo "warning: $(1) is not installed, skipping this check" >&2; test -z "$$CI"; exit; \
	fi;

SHELL_SCRIPTS = $(shell find . \( -name .git -o -name .claude -o -name .direnv -o -name build \
	  -o -name target \) -prune -o -type f \( -name '*.sh' -o -name '*.bash' \) -print) \
	$(wildcard .githooks/*)

.PHONY: fmt
fmt: ## Format all Rust code
	cargo fmt --all
	cd programs && cargo fmt --all

.PHONY: check-fmt
check-fmt: ## Check formatting of all Rust code
	cargo fmt --all --check
	cd programs && cargo fmt --all --check

.PHONY: check-shellcheck
check-shellcheck: ## Run shellcheck on every shell script
	@$(call need,shellcheck) echo shellcheck $(SHELL_SCRIPTS) && shellcheck $(SHELL_SCRIPTS)

# mdsh versions disagree on details of their output (e.g. blank lines after an
# include), so only the devShell's mdsh is used, through `nix develop` when
# make isn't already running inside the devShell.
ifdef IN_NIX_SHELL
CHECK_MDSH := @$(call need,mdsh) scripts/check-mdsh.bash
else
CHECK_MDSH := @$(call need,nix) nix develop ./nix\#build -c scripts/check-mdsh.bash
endif

.PHONY: check-mdsh
check-mdsh: ## Check the mdsh-generated sections of the docs with the devShell's mdsh
	$(CHECK_MDSH)

.PHONY: check-actionlint
check-actionlint: ## Lint the GitHub Actions workflows
	@$(call need,actionlint) actionlint

.PHONY: clippy
# The kernel is linted for its own target (on the host, its interrupt handlers
# aren't built), the rest of its workspace for the host's.
clippy: $(PROGRAMS) ## Run clippy on the kernel and the user programs, denying warnings
	cargo clippy --target $(KERNEL_TARGET) --package kidneyos --package kidneyos-shared -- -D warnings
	cargo clippy --target x86_64-unknown-linux-gnu --workspace --exclude kidneyos -- -D warnings
	$(PROGRAMS_CARGO) clippy --workspace -- -D warnings

.PHONY: check-fast
check-fast: check-fmt check-shellcheck check-mdsh check-actionlint ## Quick checks (run by the pre-commit hook)

.PHONY: check
check: check-fast programs clippy test iso ## Everything CI checks: check-fast, clippy, tests and the ISO

# The commits a pull request adds; CI sets this to the PR's base..head.
COMMIT_RANGE ?= origin/main..HEAD

.PHONY: check-commits
check-commits: ## Check the messages of the commits in COMMIT_RANGE (default origin/main..HEAD)
	build-support/check-commit-messages.bash '$(COMMIT_RANGE)'

.PHONY: hooks
hooks: ## Use the git hooks in .githooks (pre-commit and commit-msg)
	git config core.hooksPath .githooks
	@echo "Git hooks installed from .githooks"

# Misc

.PHONY: clean
clean:: ## Remove all build outputs
	cargo clean
	rm -rf build

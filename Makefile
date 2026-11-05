# Run `make help` for a list of targets.

PROJECT_DIR := $(patsubst %/,%,$(dir $(realpath $(lastword $(MAKEFILE_LIST)))))

.DEFAULT_GOAL := iso

-include local.mk

CARGO_TARGET_DIR ?= $(PROJECT_DIR)/build/target
PROFILE ?= dev
# The architecture to build the kernel for: i686, or x86_64 while the port to it
# is in progress (so far it only boots; see kernel/src/arch/x86_64).
ARCH ?= i686
# Prefix of the i686 cross binutils and gcc.
CROSS_PREFIX ?= i686-unknown-linux-gnu-
# Override in local.mk if your grub-mkrescue has a different name (e.g. Homebrew's
# i686-elf-grub-mkrescue on macOS).
GRUB_MKRESCUE ?= grub-mkrescue

export CARGO_TARGET_DIR

ifeq ($(ARCH),i686)
KERNEL_TARGET := i686-unknown-kernel
QEMU := qemu-system-i386
OBJCOPY := $(CROSS_PREFIX)objcopy
# What the smoke test waits for on the serial port: the shell prompt.
SMOKE_EXPECT := kidney:/$$
else ifeq ($(ARCH),x86_64)
KERNEL_TARGET := x86_64-unknown-none
QEMU := qemu-system-x86_64
# The i686 binutils can't be relied on to handle ELF64, so use the llvm-objcopy
# of the llvm-tools component.
OBJCOPY := $(shell rustc --print sysroot)/lib/rustlib/$(shell rustc --print host-tuple)/bin/llvm-objcopy
SMOKE_EXPECT := KidneyOS x86_64: hello
else
$(error Unhandled ARCH: $(ARCH) (expected i686 or x86_64))
endif

ifeq ($(PROFILE),dev)
ARTIFACT_DIR := $(CARGO_TARGET_DIR)/$(KERNEL_TARGET)/debug
else ifeq ($(PROFILE),release)
ARTIFACT_DIR := $(CARGO_TARGET_DIR)/$(KERNEL_TARGET)/release
else
$(error Unhandled profile: $(PROFILE))
endif

# Everything built for one architecture (except by cargo, which already keeps
# targets apart) goes here.
BUILD_DIR := build/$(ARCH)

RAW_KERNEL := $(ARTIFACT_DIR)/kidneyos
RAW_TRAMPOLINE := $(ARTIFACT_DIR)/libkidneyos_trampoline.a
TRAMPOLINE_DIR := $(BUILD_DIR)/trampoline
TRAMPOLINE := $(TRAMPOLINE_DIR)/libkidneyos_trampoline.a
ISOFILES := $(BUILD_DIR)/isofiles
KERNEL_SYM := $(BUILD_DIR)/kernel.sym
ISO := $(BUILD_DIR)/kidneyos.iso
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
	@echo 'Variables: ARCH=i686|x86_64, PROFILE=dev|release, QEMU_DEBUG=<qemu -d items>,'
	@echo '           BOCHS_DISPLAY=<bochs display_library>'

.PHONY: iso
iso: $(ISO) ## Build the bootable ISO (build/ARCH/kidneyos.iso); the default target

# Rust Builds

ifeq ($(ARCH),i686)

$(RAW_KERNEL): kernel/Cargo.toml Cargo.lock build-support/i686.ld $(TRAMPOLINE) $(PROGRAMS) \
		$(call cargo-deps,$(ARTIFACT_DIR)/kidneyos.d)
	KIDNEYOS_TRAMPOLINE_DIR=$(PROJECT_DIR)/$(TRAMPOLINE_DIR) cargo rustc \
	  --bin kidneyos \
	  --manifest-path $< \
	  --profile $(PROFILE) \
	  --target build-support/i686-unknown-kernel.json \
	  -Z json-target-spec \
	  -Z build-std=core,alloc \
	  -Z build-std-features=compiler-builtins-mem \
	  -- \
	  -C link-arg=-T -C link-arg=build-support/i686.ld \
	  -C link-arg=-z -C link-arg=max-page-size=0x1000 \
	  -C link-arg=-n

$(RAW_TRAMPOLINE): trampoline/Cargo.toml Cargo.lock \
		$(call cargo-deps,$(ARTIFACT_DIR)/libkidneyos_trampoline.d)
	cargo build \
	  --manifest-path $< \
	  --profile $(PROFILE) \
	  --target build-support/i686-unknown-kernel.json \
	  -Z json-target-spec \
	  -Z build-std=core \
	  -Z build-std-features=compiler-builtins-mem

else ifeq ($(ARCH),x86_64)

# The built-in target is already freestanding, soft-float, without a red zone and
# uses the kernel code model, and core and alloc come prebuilt for it; the
# rustflags in .cargo/config.toml link it at a fixed address rather than as a
# static PIE. The boot stub is part of the kernel, so there is no trampoline.
$(RAW_KERNEL): kernel/Cargo.toml Cargo.lock build-support/x86_64.ld \
		$(call cargo-deps,$(ARTIFACT_DIR)/kidneyos.d)
	cargo rustc \
	  --bin kidneyos \
	  --manifest-path $< \
	  --profile $(PROFILE) \
	  --target x86_64-unknown-none \
	  -- \
	  -C link-arg=-T -C link-arg=build-support/x86_64.ld \
	  -C link-arg=-z -C link-arg=max-page-size=0x1000 \
	  -C link-arg=-n

endif

# Trampoline Post-Build

$(TRAMPOLINE_DIR)/libkidneyos_trampoline: $(RAW_TRAMPOLINE)
	rm -rf $@
	mkdir -p $@
	$(CROSS_PREFIX)ar x $< --output=$@

$(TRAMPOLINE_DIR)/libkidneyos_trampoline_unlocalized.o: $(TRAMPOLINE_DIR)/libkidneyos_trampoline
	$(CROSS_PREFIX)ld -r -o $@ $</*.o

$(TRAMPOLINE_DIR)/libkidneyos_trampoline.o: $(TRAMPOLINE_DIR)/libkidneyos_trampoline_unlocalized.o
	$(CROSS_PREFIX)objcopy --keep-global-symbol _start --rename-section .text=.trampoline.text $< $@

$(TRAMPOLINE): $(TRAMPOLINE_DIR)/libkidneyos_trampoline.o
	rm -f $@
	$(CROSS_PREFIX)ar crs $@ $<

# Kernel Post-Build

$(ISOFILES)/boot/kernel.bin: $(RAW_KERNEL)
	mkdir -p $(dir $@)
	$(OBJCOPY) --strip-debug $< $@

$(KERNEL_SYM): $(RAW_KERNEL)
	mkdir -p $(dir $@)
	$(OBJCOPY) --only-keep-debug $< $@

$(ISOFILES)/boot/grub/grub.cfg: build-support/grub-$(ARCH).cfg
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

SMOKE_TEST := build-support/qemu-smoke.bash $(QEMU) $(SMOKE_TIMEOUT) $(BUILD_DIR)/smoke-test.log \
	'$(SMOKE_EXPECT)' $(QEMU_FLAGS)

.PHONY: smoke-test
smoke-test: $(ISO) $(DISK) ## Boot in QEMU and wait for the shell prompt (x86_64: the greeting)
	$(SMOKE_TEST)

.PHONY: smoke
smoke: smoke-test ## Alias for smoke-test

# For an ISO that was built elsewhere (CI downloads it from the build job), so
# make doesn't try to rebuild it.
.PHONY: smoke-test-iso
smoke-test-iso: $(DISK) ## Like smoke-test, but boot the existing build/ARCH/kidneyos.iso as is
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
run-bochs: $(ISO) $(DISK) ## Run KidneyOS in Bochs (i686 only)
	@test '$(ARCH)' = i686 || { echo 'run-bochs only supports ARCH=i686 (see bochsrc.txt).' >&2; exit 1; }
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
TEST_BINARY_DIR := $(CARGO_TARGET_DIR)/i686-unknown-linux-gnu/debug

# The rustflags are passed with --config rather than RUSTFLAGS so that they are
# added to the ones in .cargo/config.toml instead of replacing them.
.PHONY: test
test: $(PROGRAMS) ## Run the tests (with coverage instrumentation)
	rm -rf $(COVERAGE_DIR)
	mkdir -p $(COVERAGE_DIR)
	LLVM_PROFILE_FILE='$(COVERAGE_DIR)/%p-%m.profraw' cargo test \
	  --target i686-unknown-linux-gnu --workspace \
	  --config 'target.i686-unknown-linux-gnu.rustflags = ["-C", "instrument-coverage"]'
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
ifeq ($(ARCH),i686)
clippy: $(PROGRAMS) ## Run clippy on the kernel and the user programs, denying warnings
	cargo clippy --target i686-unknown-linux-gnu --workspace -- -D warnings
	$(PROGRAMS_CARGO) clippy --workspace -- -D warnings
else
# Only the kernel (and what it uses) builds for x86_64 so far.
clippy:
	cargo clippy --target $(KERNEL_TARGET) --package kidneyos --package kidneyos-shared -- -D warnings
endif

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

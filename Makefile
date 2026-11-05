# Run `make help` for a list of targets.

PROJECT_DIR := $(patsubst %/,%,$(dir $(realpath $(lastword $(MAKEFILE_LIST)))))

.DEFAULT_GOAL := iso

-include local.mk

CARGO_TARGET_DIR ?= $(PROJECT_DIR)/build/target
PROFILE ?= dev
# Prefix of the i686 cross binutils and gcc.
CROSS_PREFIX ?= i686-unknown-linux-gnu-
# Override in local.mk if your grub-mkrescue has a different name (e.g. Homebrew's
# i686-elf-grub-mkrescue on macOS).
GRUB_MKRESCUE ?= grub-mkrescue

export CARGO_TARGET_DIR

ifeq ($(PROFILE),dev)
ARTIFACT_DIR := $(CARGO_TARGET_DIR)/i686-unknown-kernel/debug
else ifeq ($(PROFILE),release)
ARTIFACT_DIR := $(CARGO_TARGET_DIR)/i686-unknown-kernel/release
else
$(error Unhandled profile: $(PROFILE))
endif

RAW_KERNEL := $(ARTIFACT_DIR)/kidneyos
RAW_TRAMPOLINE := $(ARTIFACT_DIR)/libkidneyos_trampoline.a
TRAMPOLINE_DIR := build/trampoline
TRAMPOLINE := $(TRAMPOLINE_DIR)/libkidneyos_trampoline.a
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
	@echo 'Variables: PROFILE=dev|release, QEMU_DEBUG=<qemu -d items>, BOCHS_DISPLAY=<bochs display_library>'

.PHONY: iso
iso: $(ISO) ## Build the bootable ISO (build/kidneyos.iso); the default target

# Rust Builds

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

build/isofiles/boot/kernel.bin: $(RAW_KERNEL)
	mkdir -p build/isofiles/boot
	$(CROSS_PREFIX)objcopy --strip-debug $< $@

build/kernel.sym: $(RAW_KERNEL)
	$(CROSS_PREFIX)objcopy --only-keep-debug $< $@

build/isofiles/boot/grub/grub.cfg: build-support/grub.cfg
	mkdir -p build/isofiles/boot/grub
	cp $< $@

$(ISO): build/isofiles/boot/kernel.bin build/isofiles/boot/grub/grub.cfg
	$(GRUB_MKRESCUE) -o $@ build/isofiles

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
# `qemu-system-i386 -d help` for the full list.
QEMU_DEBUG ?=
QEMU_FLAGS := -no-reboot -no-shutdown -m 4G -cdrom $(ISO) \
	-drive format=raw,file=$(DISK),if=ide \
	-boot d \
	-cpu Haswell,+rdrand \
	$(if $(QEMU_DEBUG),-d $(QEMU_DEBUG))

.PHONY: run-qemu
run-qemu: $(ISO) $(DISK) ## Run KidneyOS in QEMU
	qemu-system-i386 $(QEMU_FLAGS)

.PHONY: run-qemu-gdb
run-qemu-gdb: $(ISO) $(DISK) build/kernel.sym ## Run in QEMU, paused until GDB attaches on :1234
	qemu-system-i386 -s -S $(QEMU_FLAGS)

.PHONY: run-qemu-ng
run-qemu-ng: $(ISO) $(DISK) ## Run in QEMU without a display (serial on the terminal)
	# NOTE: You can quit with Ctrl-A X
	qemu-system-i386 -nographic $(QEMU_FLAGS)

.PHONY: run-qemu-ng-gdb
run-qemu-ng-gdb: $(ISO) $(DISK) build/kernel.sym ## run-qemu-ng, paused until GDB attaches on :1234
	# NOTE: You can quit with Ctrl-A X
	qemu-system-i386 -nographic -s -S $(QEMU_FLAGS)

SMOKE_TIMEOUT ?= 120

.PHONY: smoke-test
smoke-test: $(ISO) $(DISK) ## Boot in QEMU and wait for the shell prompt
	build-support/qemu-smoke.bash $(SMOKE_TIMEOUT) build/smoke-test.log $(QEMU_FLAGS)

# Bochs' display library. The X11 debugger GUI is only available on Linux;
# elsewhere Bochs' default is used.
ifeq ($(shell uname -s),Linux)
BOCHS_DISPLAY ?= x, options="gui_debug"
else
BOCHS_DISPLAY ?=
endif

.PHONY: run-bochs
run-bochs: $(ISO) $(DISK) ## Run KidneyOS in Bochs
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

.PHONY: check-mdsh
check-mdsh: ## Check that mdsh-generated sections of the docs are up to date
	@$(call need,mdsh) scripts/check-mdsh.bash

.PHONY: check-actionlint
check-actionlint: ## Lint the GitHub Actions workflows
	@$(call need,actionlint) actionlint

.PHONY: clippy
clippy: $(PROGRAMS) ## Run clippy on the kernel and the user programs, denying warnings
	cargo clippy --target i686-unknown-linux-gnu --workspace -- -D warnings
	$(PROGRAMS_CARGO) clippy --workspace -- -D warnings

.PHONY: check-fast
check-fast: check-fmt check-shellcheck check-mdsh check-actionlint ## Quick checks (run by the pre-commit hook)

.PHONY: check
check: check-fast programs clippy test iso ## Everything CI checks: check-fast, clippy, tests and the ISO

.PHONY: hooks
hooks: ## Use the git hooks in .githooks (pre-commit and commit-msg)
	git config core.hooksPath .githooks
	@echo "Git hooks installed from .githooks"

# Misc

.PHONY: clean
clean:: ## Remove all build outputs
	cargo clean
	rm -rf build

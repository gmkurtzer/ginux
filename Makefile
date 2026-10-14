SHELL := /bin/bash

TARGET := x86_64-unknown-uefi
PROFILE ?= release
CARGO_PROFILE_FLAG := $(if $(filter release,$(PROFILE)),--release,)
EFI_SOURCE := target/$(TARGET)/$(PROFILE)/ginux-boot.efi
ESP_DIR := build/esp
EFI_DESTINATION := $(ESP_DIR)/EFI/BOOT/BOOTX64.EFI
LINUX_PROBE := build/linux-probe.elf
LINUX_DYNAMIC_DIR := build/linux-dynamic
VOLUME_SERVICE := target/x86_64-unknown-none/$(PROFILE)/ginux-volume-service-runtime
CONTROLLER_RUNTIME := target/x86_64-unknown-none/$(PROFILE)/ginux-controller-runtime
NETWORK_SERVICE := target/x86_64-unknown-none/$(PROFILE)/ginux-network-service-runtime
PROTOCOL_SERVICE := target/x86_64-unknown-none/$(PROFILE)/ginux-protocol-service-runtime
INITRAMFS := build/initramfs-v0.cpio
SYSTEM_PROFILE := profiles/minimal-system-v0/profile.tsv

.DEFAULT_GOAL := help

.PHONY: help doctor manifest system-image alpine-image alpine-image-check build-alpine run-alpine test-alpine rocky-image rocky-image-check linux-probe controller-runtime network-service protocol-service volume-service abi-conformance architecture-check build run test test-c1 test-c2 fmt gate clean

help:
	@echo "Ginux development targets"
	@echo
	@echo "  make doctor  Check the local development prerequisites"
	@echo "  make manifest Regenerate the signed development boot inputs"
	@echo "  make system-image Build the pinned minimal system-container candidate"
	@echo "  make alpine-image Prepare and verify the pinned Alpine x86-64 OCI image"
	@echo "  make alpine-image-check Verify the prepared Alpine image without downloading"
	@echo "  make build-alpine Build Ginux with the verified Alpine rootfs sealed in"
	@echo "  make run-alpine Compatibility alias for 'make run'"
	@echo "  make test-alpine Verify the Alpine OCI boot ingestion path in QEMU"
	@echo "  make rocky-image Fetch and export the pinned Rocky 10 x86-64 system image"
	@echo "  make rocky-image-check Verify the prepared Rocky image without downloading"
	@echo "  make linux-probe Build the static probe and dynamic Linux ELF fixtures"
	@echo "  make controller-runtime Build the embedded ring-3 controller"
	@echo "  make network-service Build the freestanding isolated network service"
	@echo "  make protocol-service Build the freestanding isolated protocol service"
	@echo "  make volume-service Build the freestanding isolated volume service"
	@echo "  make abi-conformance Compile the public native ABI as freestanding C"
	@echo "  make architecture-check Check architecture and roadmap consistency"
	@echo "  make build   Build the x86-64 UEFI image"
	@echo "  make run     Prepare Alpine, build Ginux, and boot it in headless QEMU"
	@echo "  make test    Build and verify the automated QEMU boot test"
	@echo "  make test-c1 Build and verify the interactive controller console"
	@echo "  make test-c2 Verify configured container lifecycle management"
	@echo "  make check   Run formatting and lint checks"
	@echo "  make fmt     Format Rust sources"
	@echo "  make gate    Run the clean reproducible build and QEMU test gate"
	@echo "  make clean   Remove generated build artifacts"

doctor:
	@tools/qemu/doctor.sh

manifest: linux-probe controller-runtime network-service protocol-service volume-service
	@mkdir -p "$(dir $(INITRAMFS))"
	@cargo run --quiet --package ginux-manifest-tool -- boot/manifest-v0.conf control-plane/bootstrap.hex "$(INITRAMFS)" "$(LINUX_DYNAMIC_DIR)" "$(CONTROLLER_RUNTIME)" "$(NETWORK_SERVICE)" "$(PROTOCOL_SERVICE)" "$(VOLUME_SERVICE)" "$(SYSTEM_PROFILE)" boot/efi/src/generated_boot.rs
	@test -s "$(INITRAMFS)"

system-image: manifest
	@echo "Built $(INITRAMFS) from $(SYSTEM_PROFILE)"

alpine-image:
	@tools/images/prepare-alpine.sh fetch

alpine-image-check:
	@tools/images/prepare-alpine.sh check

rocky-image:
	@tools/images/prepare-rocky.sh fetch

rocky-image-check:
	@tools/images/prepare-rocky.sh check

linux-probe:
	@mkdir -p "$(dir $(LINUX_PROBE))"
	@GINUX_LINUX_PROBE_EXPORT="$(abspath $(LINUX_PROBE))" GINUX_LINUX_DYNAMIC_EXPORT_DIR="$(abspath $(LINUX_DYNAMIC_DIR))" cargo build --package ginux-kernel --target $(TARGET) $(CARGO_PROFILE_FLAG)
	@test -f "$(LINUX_PROBE)"
	@test -f "$(LINUX_DYNAMIC_DIR)/linux-linked-main.elf"
	@test -f "$(LINUX_DYNAMIC_DIR)/linux-linked-graph-main.elf"
	@test -f "$(LINUX_DYNAMIC_DIR)/libginux.so"
	@test -f "$(LINUX_DYNAMIC_DIR)/libginux-aux.so"
	@test -f "$(LINUX_DYNAMIC_DIR)/ld-ginux.so"
	@test -f "$(LINUX_DYNAMIC_DIR)/linux-exec-second.elf"
	@test -f "$(LINUX_DYNAMIC_DIR)/linux-signal-probe.elf"
	@test -f "$(LINUX_DYNAMIC_DIR)/linux-system-init.elf"
	@echo "Built $(LINUX_PROBE) and $(LINUX_DYNAMIC_DIR)"

volume-service:
	@cargo build --package ginux-volume-service-runtime --target x86_64-unknown-none $(CARGO_PROFILE_FLAG)
	@test -f "$(VOLUME_SERVICE)"
	@echo "Built $(VOLUME_SERVICE)"

controller-runtime:
	@cargo build --package ginux-controller-runtime --target x86_64-unknown-none $(CARGO_PROFILE_FLAG)
	@test -f "$(CONTROLLER_RUNTIME)"
	@echo "Built $(CONTROLLER_RUNTIME)"

network-service:
	@cargo build --package ginux-network-service-runtime --target x86_64-unknown-none $(CARGO_PROFILE_FLAG)
	@test -f "$(NETWORK_SERVICE)"
	@echo "Built $(NETWORK_SERVICE)"

protocol-service:
	@cargo build --package ginux-protocol-service-runtime --target x86_64-unknown-none $(CARGO_PROFILE_FLAG)
	@test -f "$(PROTOCOL_SERVICE)"
	@echo "Built $(PROTOCOL_SERVICE)"

abi-conformance:
	@clang -target x86_64-none-elf -std=c11 -ffreestanding -fsyntax-only \
		-Wall -Wextra -Werror -I. tests/abi/native-service-v0.c
	@echo "Native service ABI C conformance passed"

build: manifest
	@echo "Building Ginux for $(TARGET) ($(PROFILE))"
	@GINUX_LINUX_PROBE_EXPORT="$(abspath $(LINUX_PROBE))" GINUX_LINUX_DYNAMIC_EXPORT_DIR="$(abspath $(LINUX_DYNAMIC_DIR))" cargo build --package ginux-boot --target $(TARGET) $(CARGO_PROFILE_FLAG)
	@mkdir -p "$(dir $(EFI_DESTINATION))"
	@cp "$(EFI_SOURCE)" "$(EFI_DESTINATION)"
	@echo "Built $(EFI_DESTINATION)"

build-alpine: alpine-image manifest
	@echo "Building Alpine-enabled Ginux for $(TARGET) ($(PROFILE))"
	@GINUX_ALPINE_ROOTFS="$(abspath build/alpine-3.24-shell-v0/rootfs.tar)" cargo build --package ginux-boot --target $(TARGET) $(CARGO_PROFILE_FLAG) --features alpine-fixture
	@mkdir -p "$(dir $(EFI_DESTINATION))"
	@cp "$(EFI_SOURCE)" "$(EFI_DESTINATION)"
	@echo "Built $(EFI_DESTINATION) with pinned Alpine 3.24.2 x86-64 rootfs"

run-alpine: run

test-alpine: build-alpine
	@tests/qemu/alpine-image.sh

run: build-alpine
	@tools/qemu/run.sh

test: build
	@tests/qemu/boot.sh

test-c1: build
	@tests/qemu/controller-console.sh

test-c2: test-c1
	@tests/config/container-config.sh

architecture-check:
	@bash tools/architecture/check.sh

check: manifest architecture-check
	@$(MAKE) abi-conformance
	@cargo fmt --all -- --check
	@cargo clippy --package ginux-boot --target $(TARGET) -- -D warnings
	@cargo clippy --package ginux-native-abi -- -D warnings
	@cargo test --package ginux-native-abi
	@cargo clippy --package ginux-command -- -D warnings
	@cargo test --package ginux-command
	@cargo clippy --package ginux-controller-runtime --target x86_64-unknown-none -- -D warnings
	@cargo clippy --package ginux-protocol-state -- -D warnings
	@cargo test --package ginux-protocol-state
	@cargo clippy --package ginux-volume-format -- -D warnings
	@cargo test --package ginux-volume-format
	@cargo clippy --package ginux-volume-service -- -D warnings
	@cargo test --package ginux-volume-service
	@cargo clippy --package ginux-network-service-runtime --target x86_64-unknown-none -- -D warnings
	@cargo clippy --package ginux-protocol-service-runtime --target x86_64-unknown-none -- -D warnings
	@cargo clippy --package ginux-volume-service-runtime --target x86_64-unknown-none -- -D warnings
	@cargo clippy --package ginux-manifest-tool -- -D warnings

fmt: manifest
	@cargo fmt --all

gate:
	@$(MAKE) fmt
	@$(MAKE) clean
	@$(MAKE) check
	@$(MAKE) test-c2
	@$(MAKE) test
	@git diff --check
	@echo "Clean build and QEMU test gate passed"

clean:
	@cargo clean
	@rm -rf build
	@echo "Removed generated build artifacts"

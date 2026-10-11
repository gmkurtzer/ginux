SHELL := /bin/bash

TARGET := x86_64-unknown-uefi
PROFILE ?= release
CARGO_PROFILE_FLAG := $(if $(filter release,$(PROFILE)),--release,)
EFI_SOURCE := target/$(TARGET)/$(PROFILE)/ginux-boot.efi
ESP_DIR := build/esp
EFI_DESTINATION := $(ESP_DIR)/EFI/BOOT/BOOTX64.EFI
LINUX_PROBE := build/linux-probe.elf
LINUX_DYNAMIC_DIR := build/linux-dynamic
INITRAMFS := build/initramfs-v0.cpio

.DEFAULT_GOAL := help

.PHONY: help doctor manifest linux-probe build run test check fmt gate clean

help:
	@echo "Ginux development targets"
	@echo
	@echo "  make doctor  Check the local development prerequisites"
	@echo "  make manifest Regenerate the signed development boot inputs"
	@echo "  make linux-probe Build the static probe and dynamic Linux ELF fixtures"
	@echo "  make build   Build the x86-64 UEFI image"
	@echo "  make run     Build and boot Ginux in headless QEMU"
	@echo "  make test    Build and verify the automated QEMU boot test"
	@echo "  make check   Run formatting and lint checks"
	@echo "  make fmt     Format Rust sources"
	@echo "  make gate    Run the clean reproducible build and QEMU test gate"
	@echo "  make clean   Remove generated build artifacts"

doctor:
	@tools/qemu/doctor.sh

manifest: linux-probe
	@mkdir -p "$(dir $(INITRAMFS))"
	@cargo run --quiet --package ginux-manifest-tool -- boot/manifest-v0.conf control-plane/bootstrap.hex "$(INITRAMFS)" "$(LINUX_DYNAMIC_DIR)" boot/efi/src/generated_boot.rs
	@test -s "$(INITRAMFS)"

linux-probe:
	@mkdir -p "$(dir $(LINUX_PROBE))"
	@GINUX_LINUX_PROBE_EXPORT="$(abspath $(LINUX_PROBE))" GINUX_LINUX_DYNAMIC_EXPORT_DIR="$(abspath $(LINUX_DYNAMIC_DIR))" cargo build --package ginux-kernel --target $(TARGET) $(CARGO_PROFILE_FLAG)
	@test -f "$(LINUX_PROBE)"
	@test -f "$(LINUX_DYNAMIC_DIR)/linux-linked-main.elf"
	@test -f "$(LINUX_DYNAMIC_DIR)/linux-linked-graph-main.elf"
	@test -f "$(LINUX_DYNAMIC_DIR)/libginux.so"
	@test -f "$(LINUX_DYNAMIC_DIR)/libginux-aux.so"
	@test -f "$(LINUX_DYNAMIC_DIR)/ld-ginux.so"
	@echo "Built $(LINUX_PROBE) and $(LINUX_DYNAMIC_DIR)"

build: manifest
	@echo "Building Ginux for $(TARGET) ($(PROFILE))"
	@GINUX_LINUX_PROBE_EXPORT="$(abspath $(LINUX_PROBE))" GINUX_LINUX_DYNAMIC_EXPORT_DIR="$(abspath $(LINUX_DYNAMIC_DIR))" cargo build --package ginux-boot --target $(TARGET) $(CARGO_PROFILE_FLAG)
	@mkdir -p "$(dir $(EFI_DESTINATION))"
	@cp "$(EFI_SOURCE)" "$(EFI_DESTINATION)"
	@echo "Built $(EFI_DESTINATION)"

run: build
	@tools/qemu/run.sh

test: build
	@tests/qemu/boot.sh

check: manifest
	@cargo fmt --all -- --check
	@cargo clippy --package ginux-boot --target $(TARGET) -- -D warnings
	@cargo clippy --package ginux-manifest-tool -- -D warnings

fmt: manifest
	@cargo fmt --all

gate:
	@$(MAKE) fmt
	@$(MAKE) clean
	@$(MAKE) check
	@$(MAKE) test
	@git diff --check
	@echo "Clean build and QEMU test gate passed"

clean:
	@cargo clean
	@rm -rf build
	@echo "Removed generated build artifacts"

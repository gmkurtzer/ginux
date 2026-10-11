SHELL := /bin/bash

TARGET := x86_64-unknown-uefi
PROFILE ?= release
CARGO_PROFILE_FLAG := $(if $(filter release,$(PROFILE)),--release,)
EFI_SOURCE := target/$(TARGET)/$(PROFILE)/ginux-boot.efi
ESP_DIR := build/esp
EFI_DESTINATION := $(ESP_DIR)/EFI/BOOT/BOOTX64.EFI

.DEFAULT_GOAL := help

.PHONY: help doctor manifest build run test check fmt clean

help:
	@echo "Ginux development targets"
	@echo
	@echo "  make doctor  Check the local development prerequisites"
	@echo "  make manifest Regenerate the signed development boot inputs"
	@echo "  make build   Build the x86-64 UEFI image"
	@echo "  make run     Build and boot Ginux in headless QEMU"
	@echo "  make test    Build and verify the automated QEMU boot test"
	@echo "  make check   Run formatting and lint checks"
	@echo "  make fmt     Format Rust sources"
	@echo "  make clean   Remove generated build artifacts"

doctor:
	@tools/qemu/doctor.sh

manifest:
	@cargo run --quiet --package ginux-manifest-tool -- boot/manifest-v0.conf control-plane/bootstrap.hex boot/efi/src/generated_boot.rs

build: manifest
	@echo "Building Ginux for $(TARGET) ($(PROFILE))"
	@cargo build --package ginux-boot --target $(TARGET) $(CARGO_PROFILE_FLAG)
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

clean:
	@cargo clean
	@rm -rf build
	@echo "Removed generated build artifacts"

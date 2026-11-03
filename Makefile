SHELL := /bin/sh

X86_TARGET := x86_64-unknown-linux-gnu
BUILD_DIR := build
GINUS := target/$(X86_TARGET)/release/ginus
PAYLOAD := target/$(X86_TARGET)/release/managed-payload
BASELINE := target/$(X86_TARGET)/release/baseline-init

.PHONY: doctor fetch-test-assets build check test test-unit test-intercept bench-baseline demo-gate-1 clean

doctor:
	@./scripts/doctor.sh

fetch-test-assets:
	@./scripts/fetch-test-assets.sh

build:
	cargo build -p ginus-init --target $(X86_TARGET) --release
	cargo build -p managed-payload --target $(X86_TARGET) --release
	cargo build -p baseline-init --target $(X86_TARGET) --release
	@./scripts/build-initramfs.sh

check:
	cargo fmt --check
	cargo clippy -p ginus-core --all-targets -- -D warnings
	cargo clippy -p ginus-init --bin ginus --target $(X86_TARGET) --release -- -D warnings
	cargo clippy -p managed-payload --bin managed-payload --target $(X86_TARGET) --release -- -D warnings
	cargo clippy -p baseline-init --bin baseline-init --target $(X86_TARGET) --release -- -D warnings

test-unit:
	cargo test -p ginus-core

test-intercept: build
	@./scripts/test-intercept.sh

test: check test-unit test-intercept

bench-baseline: build
	@./scripts/bench-baseline.sh

demo-gate-1: doctor build
	@./scripts/demo-gate-1.sh

clean:
	cargo clean
	@./scripts/clean-build.sh

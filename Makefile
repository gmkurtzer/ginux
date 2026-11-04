SHELL := /bin/sh

X86_TARGET := x86_64-unknown-linux-gnu
BUILD_DIR := build
GINUS := target/$(X86_TARGET)/release/ginus
PAYLOAD := target/$(X86_TARGET)/release/managed-payload
BASELINE := target/$(X86_TARGET)/release/baseline-runner

.PHONY: doctor host-image-fetch host-image-verify host-boot-test build check test test-unit test-intercept bench-baseline demo-gate-1 clean

doctor:
	@./scripts/doctor.sh

host-image-fetch:
	@./scripts/fetch-rocky-host.sh

host-image-verify:
	@./scripts/verify-rocky-host.sh

host-boot-test: build
	@./scripts/with-rocky-host.sh ./scripts/check-rocky-host.sh

build:
	cargo build -p ginus-init --target $(X86_TARGET) --release
	cargo build -p managed-payload --target $(X86_TARGET) --release
	cargo build -p baseline-runner --target $(X86_TARGET) --release

check:
	cargo fmt --check
	cargo clippy -p ginus-core --all-targets -- -D warnings
	cargo clippy -p ginus-init --bin ginus --target $(X86_TARGET) --release -- -D warnings
	cargo clippy -p managed-payload --bin managed-payload --target $(X86_TARGET) --release -- -D warnings
	cargo clippy -p baseline-runner --bin baseline-runner --target $(X86_TARGET) --release -- -D warnings

test-unit:
	cargo test -p ginus-core

test-intercept: build
	@./scripts/with-rocky-host.sh ./scripts/test-intercept.sh

test: check test-unit test-intercept

bench-baseline: build
	@./scripts/with-rocky-host.sh ./scripts/bench-baseline.sh

demo-gate-1: doctor build
	@./scripts/with-rocky-host.sh ./scripts/demo-gate-1.sh

clean:
	cargo clean
	@./scripts/clean-build.sh

SHELL := /bin/sh

X86_TARGET := x86_64-unknown-linux-gnu
BUILD_DIR := build
GINUS := target/$(X86_TARGET)/release/ginus
PAYLOAD := target/$(X86_TARGET)/release/managed-payload
BASELINE := target/$(X86_TARGET)/release/baseline-runner
RUNTIME_FIXTURE := target/$(X86_TARGET)/release/runtime-fixture

.PHONY: doctor host-image-fetch host-image-verify host-boot-test image-helper-fetch image-prepare test-image image-cache-clean build check test test-unit test-intercept test-runtime bench-baseline demo-gate-1 demo-gate-2 demo-gate-3 clean

doctor:
	@./scripts/doctor.sh

host-image-fetch:
	@./scripts/fetch-rocky-host.sh

host-image-verify:
	@./scripts/verify-rocky-host.sh

host-boot-test: build
	@./scripts/with-rocky-host.sh ./scripts/check-rocky-host.sh

image-helper-fetch:
	@./scripts/fetch-rocky-domain-helper.sh

image-prepare:
	@./scripts/prepare-rocky-domain.sh

test-image:
	@./scripts/test-rocky-domain.sh

image-cache-clean:
	rm -rf build/cache/domain

build:
	cargo build -p ginus-init --target $(X86_TARGET) --release
	cargo build -p managed-payload --target $(X86_TARGET) --release
	cargo build -p baseline-runner --target $(X86_TARGET) --release
	cargo build -p runtime-fixture --target $(X86_TARGET) --release

check:
	cargo fmt --check
	cargo clippy -p ginus-core --all-targets -- -D warnings
	cargo clippy -p ginus-init --bin ginus --target $(X86_TARGET) --release -- -D warnings
	cargo clippy -p managed-payload --bin managed-payload --target $(X86_TARGET) --release -- -D warnings
	cargo clippy -p baseline-runner --bin baseline-runner --target $(X86_TARGET) --release -- -D warnings
	cargo clippy -p ginus-runtime --all-targets -- -D warnings
	cargo clippy -p runtime-fixture --bin runtime-fixture --target $(X86_TARGET) --release -- -D warnings

test-unit:
	cargo test -p ginus-core
	cargo test -p ginus-runtime

test-intercept: build
	@./scripts/with-rocky-host.sh ./scripts/test-intercept.sh

test-runtime: build
	@./scripts/with-rocky-host.sh ./scripts/test-runtime.sh

test: check test-unit test-intercept test-runtime test-image

bench-baseline: build
	@./scripts/with-rocky-host.sh ./scripts/bench-baseline.sh

demo-gate-1: doctor build
	@./scripts/with-rocky-host.sh ./scripts/demo-gate-1.sh

demo-gate-2: doctor
	@./scripts/demo-gate-2.sh

demo-gate-3: doctor build
	@./scripts/with-rocky-host.sh ./scripts/demo-gate-3.sh

clean:
	cargo clean
	@./scripts/clean-build.sh

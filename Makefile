SHELL := /bin/sh

X86_TARGET := x86_64-unknown-linux-gnu
BUILD_DIR := build
GINUS := target/$(X86_TARGET)/release/ginus
PAYLOAD := target/$(X86_TARGET)/release/managed-payload
BASELINE := target/$(X86_TARGET)/release/baseline-runner
RUNTIME_FIXTURE := target/$(X86_TARGET)/release/runtime-fixture

.PHONY: doctor host-image-fetch host-image-verify host-boot-test image-helper-fetch image-prepare domain-image-fetch domain-image-verify domain-image-prepare test-image test-oci-image test-oci-layers test-vfs test-native-vfs image-cache-clean build check test test-unit test-intercept test-runtime test-config test-domain test-process test-devices test-block test-network bench-baseline bench-block bench-network demo-gate-1 demo-gate-2 demo-gate-3 demo-gate-4 demo-gate-5 demo-gate-6 clean

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

domain-image-fetch:
	@./scripts/fetch-rocky-oci.sh

domain-image-verify:
	@./scripts/verify-rocky-oci.sh

domain-image-prepare:
	@./scripts/prepare-rocky-oci.sh

test-image:
	@./scripts/test-rocky-domain.sh

test-oci-image:
	@./scripts/test-rocky-oci.sh

test-oci-layers:
	@./scripts/test-oci-layers.sh

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
	cargo clippy -p ginus-domain --all-targets -- -D warnings
	cargo clippy -p ginus-devices --all-targets -- -D warnings
	cargo clippy -p ginus-init --bin ginus --target $(X86_TARGET) --release -- -D warnings
	cargo clippy -p ginus-process --all-targets -- -D warnings
	cargo clippy -p managed-payload --bin managed-payload --target $(X86_TARGET) --release -- -D warnings
	cargo clippy -p baseline-runner --bin baseline-runner --target $(X86_TARGET) --release -- -D warnings
	cargo clippy -p ginus-runtime --all-targets -- -D warnings
	cargo clippy -p ginus-vfs --all-targets -- -D warnings
	cargo clippy -p ginus-vfs-index --all-targets -- -D warnings
	cargo clippy -p runtime-fixture --bin runtime-fixture --target $(X86_TARGET) --release -- -D warnings

test-unit:
	cargo test -p ginus-core
	cargo test -p ginus-domain
	cargo test -p ginus-devices
	cargo test -p ginus-runtime
	cargo test -p ginus-process
	cargo test -p ginus-vfs

test-intercept: build
	@./scripts/with-rocky-host.sh ./scripts/test-intercept.sh

test-runtime: build
	@./scripts/with-rocky-host.sh ./scripts/test-runtime.sh

test-config: build
	@./scripts/with-rocky-host.sh ./scripts/test-config.sh

test-domain: build
	@./scripts/with-rocky-host.sh ./scripts/test-domain.sh

test-process:
	cargo test -p ginus-process

test-devices: build
	cargo test -p ginus-devices registry
	cargo test -p ginus-devices pseudo_devices
	cargo test -p ginus-devices conflicts_ioctls
	@./scripts/with-rocky-host.sh ./scripts/test-devices.sh

test-block:
	cargo test -p ginus-devices block_

test-network:
	cargo test -p ginus-devices network_

test-vfs: build domain-image-prepare
	@./scripts/with-rocky-host.sh ./scripts/test-vfs.sh

test-native-vfs: build domain-image-prepare
	@./scripts/with-rocky-host.sh ./scripts/test-native-vfs.sh

test: check test-unit test-intercept test-runtime test-config test-domain test-image test-devices

bench-baseline: build
	@./scripts/with-rocky-host.sh ./scripts/bench-baseline.sh

bench-block: build
	@./scripts/with-rocky-host.sh ./scripts/bench-devices.sh block

bench-network: build
	@./scripts/with-rocky-host.sh ./scripts/bench-devices.sh network

demo-gate-1: doctor build
	@./scripts/with-rocky-host.sh ./scripts/demo-gate-1.sh

demo-gate-2: doctor
	@./scripts/demo-gate-2.sh

demo-gate-3: doctor build
	@./scripts/with-rocky-host.sh ./scripts/demo-gate-3.sh

demo-gate-4: doctor build
	@./scripts/with-rocky-host.sh ./scripts/demo-gate-4.sh

demo-gate-5: doctor build domain-image-prepare
	@./scripts/with-rocky-host.sh ./scripts/demo-gate-5.sh

demo-gate-6: doctor build
	@./scripts/with-rocky-host.sh ./scripts/demo-gate-6.sh

clean:
	cargo clean
	@./scripts/clean-build.sh

SHELL := /bin/sh

X86_TARGET := x86_64-unknown-linux-gnu
BUILD_DIR := build
GINUX := target/$(X86_TARGET)/release/ginux
PAYLOAD := target/$(X86_TARGET)/release/managed-payload
BASELINE := target/$(X86_TARGET)/release/baseline-runner
RUNTIME_FIXTURE := target/$(X86_TARGET)/release/runtime-fixture
PROCESS_NATIVE_FIXTURE := target/$(X86_TARGET)/release/process-native-fixture

.PHONY: doctor host-image-fetch host-image-verify host-boot-test image-helper-fetch image-prepare domain-image-fetch domain-image-verify domain-image-prepare systemd-image-fetch systemd-image-verify systemd-image-prepare test-image test-oci-image test-oci-layers test-vfs test-native-vfs image-cache-clean build check-name check test test-unit test-intercept test-runtime test-config test-domain test-process build-process-fixtures test-process-native test-devices test-kernel-views test-systemd test-systemd-live test-systemd-interactive test-gate975-network test-cgroup test-block test-network run-host run-systemd bench bench-baseline bench-block bench-network demo-gate-1 demo-gate-2 demo-gate-3 demo-gate-4 demo-gate-5 demo-gate-6 demo-gate-7 demo-gate-8 demo-gate-9 demo-gate-9.5 clean

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

systemd-image-fetch:
	@GINUX_OCI_CONFIG_FILE=./config/rocky-systemd-oci.env ./scripts/fetch-rocky-oci.sh

systemd-image-verify:
	@GINUX_OCI_CONFIG_FILE=./config/rocky-systemd-oci.env ./scripts/verify-rocky-oci.sh

systemd-image-prepare:
	@GINUX_OCI_CONFIG_FILE=./config/rocky-systemd-oci.env ./scripts/prepare-rocky-oci.sh

test-image:
	@./scripts/test-rocky-domain.sh

test-oci-image:
	@./scripts/test-rocky-oci.sh

test-oci-layers:
	@./scripts/test-oci-layers.sh

image-cache-clean:
	rm -rf build/cache/domain

build:
	cargo build -p ginux-init --target $(X86_TARGET) --release
	cargo build -p managed-payload --target $(X86_TARGET) --release
	cargo build -p baseline-runner --target $(X86_TARGET) --release
	cargo build -p runtime-fixture --target $(X86_TARGET) --release
	cargo build -p process-native-fixture --target $(X86_TARGET) --release

build-process-fixtures:
	mkdir -p .artifacts/process
	@for scenario in syscall_abi untraced_clone shared_pipe wait_status thread_reuse denied_host_memory inherited_fds thread_exec unused_parent_tid exec_failure copied_files shared_exec clone_tid child_signal nonblocking_wait vfork concurrent_wait remaining_wait sigpipe segv kill_badsig thread_segv thread_term parked_kill parked_term ro_status bad_status bad_rusage straddle_status alias_writes; do \
		$(CC) -nostdlib -static -fno-stack-protector -fno-builtin -O2 -DCASE_$$scenario tests/fixtures/process-review-fixture.c -o .artifacts/process/$$scenario || exit; \
	done
	$(CC) -nostdlib -static tests/fixtures/process-exec-target.S -o .artifacts/process/process-exec-target

test-process-native: build build-process-fixtures
	python3 scripts/test-process-native.py $(GINUX) .artifacts/process $(PROCESS_NATIVE_FIXTURE) $(SCENARIO)

check-name:
	@./scripts/check-name.sh

check: check-name
	cargo fmt --check
	cargo clippy -p ginux-core --all-targets -- -D warnings
	cargo clippy -p ginux-domain --all-targets -- -D warnings
	cargo clippy -p ginux-devices --all-targets -- -D warnings
	cargo clippy -p ginux-init --bin ginux --target $(X86_TARGET) --release -- -D warnings
	cargo clippy -p ginux-kernel-views --all-targets -- -D warnings
	cargo clippy -p ginux-process --all-targets -- -D warnings
	cargo clippy -p managed-payload --bin managed-payload --target $(X86_TARGET) --release -- -D warnings
	cargo clippy -p baseline-runner --bin baseline-runner --target $(X86_TARGET) --release -- -D warnings
	cargo clippy -p ginux-runtime --all-targets -- -D warnings
	cargo clippy -p ginux-vfs --all-targets -- -D warnings
	cargo clippy -p ginux-vfs-index --all-targets -- -D warnings
	cargo clippy -p runtime-fixture --bin runtime-fixture --target $(X86_TARGET) --release -- -D warnings
	cargo clippy -p process-native-fixture --bin process-native-fixture --target $(X86_TARGET) --release -- -D warnings

test-unit:
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-core
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-domain
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-devices
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-runtime
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-process
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-vfs
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-kernel-views

test-intercept: build
	@./scripts/with-rocky-host.sh ./scripts/test-intercept.sh

.PHONY: build-gate1-fixture test-gate1-setup
build-gate1-fixture:
	mkdir -p .artifacts/gate1
	$(CC) -nostdlib -static -fno-stack-protector -fno-builtin -O2 -Wall -Wextra -Werror -DDENY_PTRACE tests/fixtures/gate1-setup-fixture.c -o .artifacts/gate1/deny-ptrace
	$(CC) -nostdlib -static -fno-stack-protector -fno-builtin -O2 -Wall -Wextra -Werror tests/fixtures/gate1-setup-fixture.c -o .artifacts/gate1/payload

test-gate1-setup:
	python3 scripts/test-gate1-setup.py $(GINUX) .artifacts/gate1/deny-ptrace .artifacts/gate1/payload

test-runtime: build
	@./scripts/with-rocky-host.sh ./scripts/test-runtime.sh

test-config: build
	@./scripts/with-rocky-host.sh ./scripts/test-config.sh

test-domain: build
	@./scripts/with-rocky-host.sh ./scripts/test-domain.sh

test-process: build
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-process
	@./scripts/with-rocky-host.sh ./scripts/test-process.sh

test-devices: build
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-devices registry
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-devices pseudo_devices
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-devices conflicts_ioctls
	@./scripts/with-rocky-host.sh ./scripts/test-devices.sh

test-kernel-views: build
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-kernel-views
	@./scripts/with-rocky-host.sh ./scripts/test-kernel-views.sh

test-systemd: build systemd-image-prepare
	@ROCKY_NETWORK_RESTRICT=on ./scripts/with-rocky-host.sh ./scripts/test-systemd.sh

test-systemd-live: build systemd-image-prepare
	@ROCKY_NETWORK_RESTRICT=off ./scripts/with-rocky-host.sh ./scripts/test-systemd-live.sh

test-systemd-interactive: build
	@./scripts/test-systemd-interactive.exp

test-gate975-network: build systemd-image-prepare
	@ROCKY_NETWORK_RESTRICT=on ./scripts/with-rocky-host.sh ./scripts/test-gate975-network.sh

test-cgroup:
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-kernel-views cgroup::

test-block:
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-devices block_

test-network:
	RUSTFLAGS='' CARGO_TARGET_DIR=.artifacts/unit cargo test -p ginux-devices network_

run-host: build
	@./scripts/run-host.sh

run-systemd: run-host

test-vfs: build domain-image-prepare
	@./scripts/with-rocky-host.sh ./scripts/test-vfs.sh

test-native-vfs: build domain-image-prepare
	@./scripts/with-rocky-host.sh ./scripts/test-native-vfs.sh

test: check test-unit test-intercept test-runtime test-config test-domain test-image test-devices test-process test-kernel-views

bench-baseline: build
	@./scripts/with-rocky-host.sh ./scripts/bench-baseline.sh

bench-block: build
	@./scripts/with-rocky-host.sh ./scripts/bench-devices.sh block

bench-network: build
	@./scripts/with-rocky-host.sh ./scripts/bench-devices.sh network

bench: build systemd-image-prepare
	@ROCKY_NETWORK_RESTRICT=on ./scripts/with-rocky-host.sh ./scripts/bench-systemd.sh

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

demo-gate-7: doctor build
	@./scripts/with-rocky-host.sh ./scripts/demo-gate-7.sh

demo-gate-9.5: doctor build
	@./scripts/test-systemd-interactive.exp
	@printf '%s\n' 'GATE9.5: PASS (create-time OCI cache, configured init, job control, procfs, attach lifecycle, shutdown)'

demo-gate-8: doctor build
	@./scripts/with-rocky-host.sh ./scripts/demo-gate-8.sh

demo-gate-9: doctor build systemd-image-prepare
	@./scripts/demo-gate-9.sh

clean:
	cargo clean
	@./scripts/clean-build.sh

.PHONY: build-launch-fixture test-launch test-native-supervision
build-launch-fixture:
	mkdir -p .artifacts/launch
	$(CC) -nostdlib -static -fno-stack-protector -fno-builtin -O2 -Wall -Wextra -Werror tests/fixtures/launch-fixture.c -o .artifacts/launch/launch-fixture
	$(CC) -nostdlib -static -fno-stack-protector -fno-builtin -O2 -Wall -Wextra -Werror tests/fixtures/native-fixture.c -o .artifacts/launch/native-fixture

test-launch:
	python3 scripts/test-launch.py $(GINUX) .artifacts/launch/launch-fixture

test-native-supervision:
	python3 scripts/test-native-supervision.py $(GINUX) .artifacts/launch/native-fixture

# Ginux

Ginux is an immutable, capability-secure, container-native operating system for AI infrastructure. The native host is intentionally small: an immutable kernel boots one constrained control-plane domain, which creates isolated services and containers through stable interfaces.

The initial executable milestone is an x86-64 unified EFI image tested under QEMU. Its boot component captures the UEFI memory map, exits firmware boot services, and transfers control to a separate `no_std` kernel crate through an explicit boot-information contract. The kernel switches to its own stack and page tables, installs its CPU protection tables, verifies a recoverable exception, and inventories usable physical memory before reporting success. On Apple Silicon development hosts it uses QEMU TCG emulation. The same Q35/OVMF profile will use KVM acceleration on x86-64 Linux.

## Quick start

Requirements:

- Rust 1.99.0 through `rustup`
- QEMU with x86-64 and OVMF support
- GNU Make

Check the local environment:

```sh
make doctor
```

Build the bootable EFI filesystem:

```sh
make build
```

The result is placed at:

```text
build/esp/EFI/BOOT/BOOTX64.EFI
```

Boot it in headless QEMU:

```sh
make run
```

Run the automated boot assertion:

```sh
make test
```

The test succeeds only after the post-firmware kernel validates the UEFI memory map and emits `GINUX_KERNEL_OK` over the serial console.

`make help` lists all supported development targets.

## Virtualization profiles

- **macOS on Apple Silicon:** QEMU x86-64 with TCG. UTM is optional for graphical inspection; the checked-in QEMU runner is authoritative.
- **Linux on x86-64:** QEMU x86-64 with KVM when `/dev/kvm` is available, falling back to TCG otherwise.

Override discovery when necessary:

```sh
QEMU_ACCEL=tcg OVMF_CODE=/path/to/OVMF_CODE.fd OVMF_VARS=/path/to/OVMF_VARS.fd make run
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design, [ADR 0001](docs/decisions/0001-native-domains-and-authority.md) for the native domain and authority model, [Boot Handoff Contract v0](specs/boot/handoff-v0.md) for the firmware-to-kernel boundary, and [Paging v0](specs/kernel/paging-v0.md) for the transitional kernel address space.

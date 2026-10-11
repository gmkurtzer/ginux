# Ginux

Ginux is an immutable, capability-secure, container-native operating system for AI infrastructure. The native host is intentionally small: an immutable kernel boots one constrained control-plane domain, which creates isolated services and containers through stable interfaces.

The initial executable milestone is an x86-64 unified EFI image tested under QEMU. Its boot component passes a development-signed authority manifest and measured control-plane bundle, exits firmware boot services, and transfers control to a separate `no_std` kernel. The kernel establishes protected CPU and memory state, seals manifest-derived capabilities into one native domain, and runs the measured W^X control-plane payload at ring 3 through a capability-checked call gate. Calibrated APIC one-shots preempt the control context to run an isolated service and its replacement generation, while bounded IPC carries the request and reply. The control plane can also create, bounded-load, stage, cancel, and retire stop-only auxiliary workloads without placing third-party code in the kernel. A separate isolated service probes and configures link, IPv4, default-gateway, and one static-route record through the first manifest-authorized network-device contract in control ABI v2; a monotonic generation makes effective configuration changes observable and rejects stale whole-state applies. Distinct read-only and write-only services prove that observation and mutation authority can be delegated independently, and a faulted network service is retired without losing mediated state or panicking the kernel. Packet I/O and a general route table remain deliberately unimplemented. On Apple Silicon development hosts it uses QEMU TCG emulation. The same Q35/OVMF profile will use KVM acceleration on x86-64 Linux.

M4 begins with a strict static x86-64 ELF64 parser profile. It is validated
during boot and materializes bounded RX/RW page images with zero-filled BSS, but
does not yet install or execute Linux binaries; see
[Linux ELF64 Profile v0](specs/kernel/linux-elf-v0.md).

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

`make build` deterministically regenerates the development manifest, bundle
digest, and Ed25519 signature from `boot/manifest-v0.conf` and
`control-plane/bootstrap.hex`; that source declares its authenticated control,
service, and restart entry labels inline. Run `make manifest` directly when
working on those inputs. Kernel and proof-workload ABI constants share the machine-readable
`specs/abi/control-v2.rs` source, and the manifest builder expands its symbolic
feature and descriptor words before signing. The checked-in key is an RFC test
identity and must not be treated as a production trust root.

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

Before committing a milestone, run the complete clean gate:

```sh
make gate
```

This formats the workspace, removes prior artifacts, runs lint checks, rebuilds
the EFI image, executes the QEMU boot assertions, and checks the working diff
for whitespace errors.

`make help` lists all supported development targets.

## Virtualization profiles

- **macOS on Apple Silicon:** QEMU x86-64 with TCG. UTM is optional for graphical inspection; the checked-in QEMU runner is authoritative.
- **Linux on x86-64:** QEMU x86-64 with KVM when `/dev/kvm` is available, falling back to TCG otherwise.

Override discovery when necessary:

```sh
QEMU_ACCEL=tcg QEMU_CPU=qemu64,+smep,+smap,+umip OVMF_CODE=/path/to/OVMF_CODE.fd OVMF_VARS=/path/to/OVMF_VARS.fd make run
```

The default virtual CPU is `qemu64,+smep,+smap,+umip`, matching the kernel's
fail-closed supervisor execution/access and user-mode instruction protection
requirements. `QEMU_CPU` may select a different model only if it still
advertises SMEP, SMAP, and UMIP.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design, [ADR 0001](docs/decisions/0001-native-domains-and-authority.md) for the native domain and authority model, [Boot Handoff Contract v3](specs/boot/handoff-v3.md) for the firmware-to-kernel boundary, and [Paging v0](specs/kernel/paging-v0.md) for the transitional kernel address space.

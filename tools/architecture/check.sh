#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
architecture="${repo_root}/ARCHITECTURE.md"
roadmap="${repo_root}/docs/ROADMAP.md"
decision="${repo_root}/docs/decisions/0002-unified-image-and-driver-boundary.md"

require_text() {
    local file="$1"
    local text="$2"
    if ! grep -Fq -- "$text" "$file"; then
        echo "architecture check: missing '$text' in ${file#"${repo_root}/"}" >&2
        exit 1
    fi
}

reject_text() {
    local file="$1"
    local text="$2"
    if grep -Fq -- "$text" "$file"; then
        echo "architecture check: obsolete statement '$text' in ${file#"${repo_root}/"}" >&2
        exit 1
    fi
}

require_text "$architecture" 'One authenticated `GINUX.EFI`'
require_text "$architecture" 'executes in ring 3'
require_text "$architecture" 'Rocky Linux 10 x86-64'
require_text "$architecture" 'stable kABI'
require_text "$architecture" 'must not preclude a future authenticated compatibility layer'
require_text "$architecture" 'Per-container loopback, interfaces, IP addresses, routes, sockets'
require_text "$decision" 'The controller is embedded as bytes but executes in ring 3.'

for checkpoint in C0 C1 C2 C3 C4 C5 C6 C7; do
    require_text "$roadmap" "## ${checkpoint}:"
done

reject_text "$architecture" 'Ginux does not expose Linux kernel internals or load unmodified Linux `.ko` files into its kernel'
reject_text "$architecture" 'No third-party code executes in the kernel address space'
reject_text "$architecture" 'their implementations do not execute in the kernel'
reject_text "$architecture" 'The first proof of concept may unpack a CPIO/initramfs'
reject_text "$architecture" 'A later isolated storage service should expose `virtio-blk`'

echo "Architecture consistency check passed"

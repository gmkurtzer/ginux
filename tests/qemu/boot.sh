#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
output_file="$(mktemp -t ginux-boot.XXXXXX)"
qemu_pid=""

cleanup() {
    if [[ -n "$qemu_pid" ]] && kill -0 "$qemu_pid" 2>/dev/null; then
        kill "$qemu_pid" 2>/dev/null || true
        wait "$qemu_pid" 2>/dev/null || true
    fi
    rm -f "$output_file"
}
trap cleanup EXIT

"$repo_root/tools/qemu/run.sh" >"$output_file" 2>&1 &
qemu_pid=$!

for _ in {1..30}; do
    if ! kill -0 "$qemu_pid" 2>/dev/null; then
        break
    fi
    sleep 1
done

if kill -0 "$qemu_pid" 2>/dev/null; then
    kill "$qemu_pid" 2>/dev/null || true
    wait "$qemu_pid" 2>/dev/null || true
    qemu_pid=""
    cat "$output_file"
    echo "Boot test failed: QEMU did not finish within 30 seconds." >&2
    exit 1
fi

if wait "$qemu_pid"; then
    qemu_status=0
else
    qemu_status=$?
fi
qemu_pid=""

cat "$output_file"

if ! grep -q '^GINUX_KERNEL_OK' "$output_file"; then
    echo "Boot test failed: the post-firmware kernel marker was not emitted." >&2
    exit 1
fi

if ! grep -Eq '^ginux: memory descriptors: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: the kernel did not receive a valid UEFI memory map." >&2
    exit 1
fi

if ! grep -q '^ginux: kernel stack active' "$output_file"; then
    echo "Boot test failed: the kernel did not switch to its owned stack." >&2
    exit 1
fi

if ! grep -q '^ginux: versioned boot handoff accepted' "$output_file"; then
    echo "Boot test failed: the versioned boot handoff was not accepted." >&2
    exit 1
fi

if ! grep -q '^ginux: loaded EFI PE image accepted' "$output_file"; then
    echo "Boot test failed: the loaded EFI image metadata was rejected." >&2
    exit 1
fi

if ! grep -Eq '^ginux: PE sections: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: the loaded EFI image has no validated PE sections." >&2
    exit 1
fi

if ! grep -Eq '^ginux: executable PE sections: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: the loaded EFI image has no executable PE section." >&2
    exit 1
fi

if ! grep -q '^ginux: ACPI root pointer validated' "$output_file"; then
    echo "Boot test failed: the ACPI root pointer was not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: signed boot manifest accepted' "$output_file"; then
    echo "Boot test failed: the signed boot manifest was not accepted." >&2
    exit 1
fi

if ! grep -q '^ginux: manifest signature rejection verified' "$output_file"; then
    echo "Boot test failed: an invalid manifest signature was not rejected." >&2
    exit 1
fi

if ! grep -q '^ginux: malformed boot manifests rejected' "$output_file"; then
    echo "Boot test failed: malformed canonical manifests were not rejected." >&2
    exit 1
fi

if ! grep -Eq '^ginux: manifest authority grants: [1-8]' "$output_file"; then
    echo "Boot test failed: the boot manifest did not provide bounded authority grants." >&2
    exit 1
fi

if ! grep -q '^ginux: control-plane bundle measurement accepted' "$output_file"; then
    echo "Boot test failed: the measured control-plane bundle was not accepted." >&2
    exit 1
fi

if ! grep -q '^ginux: kernel page tables active' "$output_file"; then
    echo "Boot test failed: the kernel did not activate its own page tables." >&2
    exit 1
fi

if ! grep -q '^ginux: NX-by-default identity map active' "$output_file"; then
    echo "Boot test failed: non-code identity mappings remain executable." >&2
    exit 1
fi

if ! grep -q '^ginux: supervisor-only higher-half direct map active' "$output_file"; then
    echo "Boot test failed: higher-half direct-map alias was not active." >&2
    exit 1
fi

if ! grep -q '^ginux: bounded direct-map translation verified' "$output_file"; then
    echo "Boot test failed: direct-map translation bounds were not verified." >&2
    exit 1
fi

if ! grep -q '^ginux: PE executable identity pages read-only' "$output_file"; then
    echo "Boot test failed: executable PE pages remain writable." >&2
    exit 1
fi

if ! grep -q '^ginux: PE image section permissions enforced' "$output_file"; then
    echo "Boot test failed: PE image section permissions were not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: boot policy inputs immutable and non-executable' "$output_file"; then
    echo "Boot test failed: boot policy inputs remain mutable or executable." >&2
    exit 1
fi

if ! grep -q '^ginux: supervisor write protection active' "$output_file"; then
    echo "Boot test failed: supervisor write protection is inactive." >&2
    exit 1
fi

if ! grep -q '^ginux: static kernel page tables sealed read-only' "$output_file"; then
    echo "Boot test failed: static kernel page tables remain writable." >&2
    exit 1
fi

if ! grep -q '^ginux: supervisor execution prevention active' "$output_file"; then
    echo "Boot test failed: supervisor execution prevention is inactive." >&2
    exit 1
fi

if ! grep -q '^ginux: supervisor access prevention active' "$output_file"; then
    echo "Boot test failed: supervisor access prevention is inactive." >&2
    exit 1
fi

if ! grep -q '^ginux: user-mode instruction prevention active' "$output_file"; then
    echo "Boot test failed: user-mode instruction prevention is inactive." >&2
    exit 1
fi

if ! grep -q '^ginux: null and bidirectional stack guard pages active' "$output_file"; then
    echo "Boot test failed: hardened null/bidirectional stack mappings are not active." >&2
    exit 1
fi

if ! grep -q '^ginux: ACPI MADT validated' "$output_file"; then
    echo "Boot test failed: ACPI MADT discovery did not complete." >&2
    exit 1
fi

if ! grep -q '^ginux: ACPI tables use higher-half direct map' "$output_file"; then
    echo "Boot test failed: ACPI table access did not use the direct map." >&2
    exit 1
fi

if ! grep -Eq '^ginux: enabled processors: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: ACPI reported no enabled processors." >&2
    exit 1
fi

if ! grep -Eq '^ginux: I/O APICs: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: ACPI reported no I/O APICs." >&2
    exit 1
fi

if ! grep -q '^ginux: GDT/TSS/IDT active' "$output_file"; then
    echo "Boot test failed: kernel CPU protection tables are not active." >&2
    exit 1
fi

if ! grep -q '^ginux: user I/O-port access denied' "$output_file"; then
    echo "Boot test failed: ring-3 I/O-port access is not denied." >&2
    exit 1
fi

if ! grep -q '^ginux: architectural exception gates active' "$output_file"; then
    echo "Boot test failed: architectural exceptions are not fully covered." >&2
    exit 1
fi

if ! grep -q '^ginux: breakpoint exception handled' "$output_file"; then
    echo "Boot test failed: the recoverable exception test did not return." >&2
    exit 1
fi

if ! grep -q '^ginux: local APIC discovered' "$output_file"; then
    echo "Boot test failed: the local APIC was not discovered." >&2
    exit 1
fi

if ! grep -q '^ginux: local APIC uses higher-half direct map' "$output_file"; then
    echo "Boot test failed: local-APIC MMIO did not use the direct map." >&2
    exit 1
fi

if ! grep -Eq '^ginux: local APIC mode: x2?APIC' "$output_file"; then
    echo "Boot test failed: the local APIC mode is invalid." >&2
    exit 1
fi

if ! grep -Eq '^ginux: local APIC physical base: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: the local APIC base was not reported." >&2
    exit 1
fi

if ! grep -q '^ginux: timer capabilities discovered' "$output_file"; then
    echo "Boot test failed: timer capabilities were not discovered." >&2
    exit 1
fi

if ! grep -Eq '^ginux: invariant TSC: (advertised|not-advertised)' "$output_file"; then
    echo "Boot test failed: invariant-TSC capability is invalid." >&2
    exit 1
fi

if ! grep -Eq '^ginux: TSC deadline timer: (supported|unsupported)' "$output_file"; then
    echo "Boot test failed: TSC-deadline capability is invalid." >&2
    exit 1
fi

if ! grep -Eq '^ginux: TSC frequency source: (architectural-ratio|nominal-processor-frequency|unavailable)' "$output_file"; then
    echo "Boot test failed: TSC frequency source is invalid." >&2
    exit 1
fi

if ! grep -q '^ginux: local APIC timer calibrated' "$output_file"; then
    echo "Boot test failed: the local APIC timer was not calibrated." >&2
    exit 1
fi

if ! grep -Eq '^ginux: APIC timer counter Hz: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: the local APIC timer frequency is invalid." >&2
    exit 1
fi

if ! grep -q '^ginux: local APIC timer interrupt handled' "$output_file"; then
    echo "Boot test failed: the local APIC timer interrupt was not handled." >&2
    exit 1
fi

if ! grep -q '^ginux: capability derivation and revocation verified' "$output_file"; then
    echo "Boot test failed: capability authority invariants failed." >&2
    exit 1
fi

if ! grep -q '^ginux: exhaustive capability attenuation verified' "$output_file"; then
    echo "Boot test failed: exhaustive capability attenuation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: cross-domain capability revocation verified' "$output_file"; then
    echo "Boot test failed: cross-domain capability revocation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: per-root delegation provenance verified' "$output_file"; then
    echo "Boot test failed: per-root delegation provenance failed." >&2
    exit 1
fi

if ! grep -q '^ginux: exhaustive delegated attenuation verified' "$output_file"; then
    echo "Boot test failed: delegated capability attenuation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: independent delegation roots verified' "$output_file"; then
    echo "Boot test failed: independent delegation roots were not preserved." >&2
    exit 1
fi

if ! grep -q '^ginux: domain lifecycle ancestry verified' "$output_file"; then
    echo "Boot test failed: domain lifecycle ancestry failed." >&2
    exit 1
fi

if ! grep -q '^ginux: exhausted generational slots skipped' "$output_file"; then
    echo "Boot test failed: exhausted generational slots were not skipped." >&2
    exit 1
fi

if ! grep -q '^ginux: per-service frame quarantines isolated' "$output_file"; then
    echo "Boot test failed: per-service frame quarantines were not isolated." >&2
    exit 1
fi

if ! grep -q '^ginux: bounded auxiliary service registry verified' "$output_file"; then
    echo "Boot test failed: auxiliary service registry invariant failed." >&2
    exit 1
fi

if ! grep -q '^ginux: service lifecycle roles separated' "$output_file"; then
    echo "Boot test failed: service lifecycle roles were not separated." >&2
    exit 1
fi

if ! grep -q '^ginux: service failure policies separated' "$output_file"; then
    echo "Boot test failed: service failure policies were not separated." >&2
    exit 1
fi

if ! grep -q '^ginux: domain resource admission policies verified' "$output_file"; then
    echo "Boot test failed: domain resource admission policies were not verified." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary service quiescence enforced' "$output_file"; then
    echo "Boot test failed: auxiliary service quiescence was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary supervision authority enforced' "$output_file"; then
    echo "Boot test failed: auxiliary supervision authority was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary retirement events sequenced' "$output_file"; then
    echo "Boot test failed: auxiliary retirement events were not sequenced." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary retirement publication atomic' "$output_file"; then
    echo "Boot test failed: auxiliary retirement publication was not atomic." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary stop authority revoked' "$output_file"; then
    echo "Boot test failed: auxiliary stop authority was not revoked." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary retirement overflow summarized' "$output_file"; then
    echo "Boot test failed: auxiliary retirement overflow was not summarized." >&2
    exit 1
fi

if ! grep -q '^ginux: stop-only auxiliary faults contained' "$output_file"; then
    echo "Boot test failed: stop-only auxiliary faults were not contained." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary fault authority revoked before publication' "$output_file"; then
    echo "Boot test failed: auxiliary fault authority was not revoked first." >&2
    exit 1
fi

if ! grep -q '^ginux: bounded service restart policy verified' "$output_file"; then
    echo "Boot test failed: service restart policy validation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: clean service exit publication atomic' "$output_file"; then
    echo "Boot test failed: clean service exit publication was not atomic." >&2
    exit 1
fi

if ! grep -q '^ginux: bounded lifecycle event queue verified' "$output_file"; then
    echo "Boot test failed: lifecycle event queue invariants failed." >&2
    exit 1
fi

if ! grep -q '^ginux: lifecycle event overflow notification verified' "$output_file"; then
    echo "Boot test failed: lifecycle event overflow notification failed." >&2
    exit 1
fi

if ! grep -q '^ginux: persistent control-plane domain sealed' "$output_file"; then
    echo "Boot test failed: persistent control-plane domain state was not sealed." >&2
    exit 1
fi

if ! grep -Eq '^ginux: control-plane domain ID: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: control-plane domain identity is invalid." >&2
    exit 1
fi

if ! grep -Eq '^ginux: IPC-only service domain ID: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: isolated service domain identity is invalid." >&2
    exit 1
fi

if ! grep -Eq '^ginux: usable memory MiB: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: the kernel did not inventory usable physical memory." >&2
    exit 1
fi

if ! grep -q '^ginux: physical frame allocator active' "$output_file"; then
    echo "Boot test failed: physical frame allocation was not verified." >&2
    exit 1
fi

if ! grep -Eq '^ginux: reclaimed boot-service frames: [1-9][0-9]*' "$output_file"; then
    echo "Boot test failed: boot-service memory was not reclaimed." >&2
    exit 1
fi

if ! grep -q '^ginux: atomic frame reservations verified' "$output_file"; then
    echo "Boot test failed: atomic physical-frame reservations failed." >&2
    exit 1
fi

if ! grep -q '^ginux: physical frames zeroed before allocation' "$output_file"; then
    echo "Boot test failed: physical-frame zero-on-allocation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: tracked frame release ownership verified' "$output_file"; then
    echo "Boot test failed: physical-frame release ownership failed." >&2
    exit 1
fi

if ! grep -q '^ginux: stale frame leases rejected' "$output_file"; then
    echo "Boot test failed: stale physical-frame leases were accepted." >&2
    exit 1
fi

if ! grep -q '^ginux: per-domain physical frame budgets enforced' "$output_file"; then
    echo "Boot test failed: per-domain physical-frame budgets were not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: isolated address space active' "$output_file"; then
    echo "Boot test failed: isolated address-space activation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: control-plane W\^X mappings active' "$output_file"; then
    echo "Boot test failed: control-plane W^X mappings are not active." >&2
    exit 1
fi

if ! grep -q '^ginux: address-space frames use higher-half direct map' "$output_file"; then
    echo "Boot test failed: address-space frame access did not use the direct map." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary address-space transaction verified' "$output_file"; then
    echo "Boot test failed: auxiliary address-space transaction failed." >&2
    exit 1
fi

if ! grep -q '^ginux: concurrent auxiliary address spaces isolated' "$output_file"; then
    echo "Boot test failed: concurrent auxiliary address spaces were not isolated." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary scheduler admission resource-gated' "$output_file"; then
    echo "Boot test failed: auxiliary scheduler admission was not resource-gated." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary address-space ownership retained' "$output_file"; then
    echo "Boot test failed: auxiliary address-space ownership was not retained." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary runtime teardown path connected' "$output_file"; then
    echo "Boot test failed: auxiliary runtime teardown was not connected." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary teardown restores kernel page tables' "$output_file"; then
    echo "Boot test failed: auxiliary teardown did not restore kernel page tables." >&2
    exit 1
fi

if ! grep -q '^ginux: bounded executable container accepted' "$output_file"; then
    echo "Boot test failed: the executable container was not accepted." >&2
    exit 1
fi

if ! grep -q '^ginux: authenticated executable entry points accepted' "$output_file"; then
    echo "Boot test failed: authenticated executable entry points were not accepted." >&2
    exit 1
fi

if ! grep -q '^ginux: malformed executable containers rejected' "$output_file"; then
    echo "Boot test failed: malformed executable containers were accepted." >&2
    exit 1
fi

if ! grep -q '^ginux: non-executable Linux GNU stack enforced' "$output_file"; then
    echo "Boot test failed: Linux GNU stack policy was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: bounded multi-page Linux ELF plan verified' "$output_file"; then
    echo "Boot test failed: bounded multi-page Linux ELF planning was not verified." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux ELF headers mapped in executable segment' "$output_file"; then
    echo "Boot test failed: Linux ELF headers were not mapped." >&2
    exit 1
fi

if ! grep -q '^ginux: bounded multi-page Linux ELF mapping verified' "$output_file"; then
    echo "Boot test failed: bounded multi-page Linux ELF mapping was not verified." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux copy errors and overflow bounds verified' "$output_file"; then
    echo "Boot test failed: Linux copy errors or overflow bounds were not verified." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux proof workload assembled from source' "$output_file"; then
    echo "Boot test failed: assembled Linux proof workload did not complete." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux container-local root credentials verified' "$output_file"; then
    echo "Boot test failed: Linux container-local root credentials were not verified." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux errno recovery verified in ring 3' "$output_file"; then
    echo "Boot test failed: Linux ring-3 errno recovery was not verified." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux single-thread process identity verified' "$output_file"; then
    echo "Boot test failed: Linux single-thread process identity was not verified." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux initial environment vector validated' "$output_file"; then
    echo "Boot test failed: Linux initial environment vector was not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux credential auxiliary vector validated' "$output_file"; then
    echo "Boot test failed: Linux credential auxiliary vector was not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux FS base query validated' "$output_file"; then
    echo "Boot test failed: Linux FS base query was not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux clear-child-TID exit action validated' "$output_file"; then
    echo "Boot test failed: Linux clear-child-TID exit action was not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux initial process namespace identity validated' "$output_file"; then
    echo "Boot test failed: Linux initial process namespace identity was not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux static executable metadata validated' "$output_file"; then
    echo "Boot test failed: Linux static executable metadata was not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux bounded brk heap validated' "$output_file"; then
    echo "Boot test failed: Linux bounded brk heap was not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux heap copy bounds validated' "$output_file"; then
    echo "Boot test failed: Linux heap copy bounds were not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux stack copy bounds validated' "$output_file"; then
    echo "Boot test failed: Linux stack copy bounds were not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux directional VMA copies validated' "$output_file"; then
    echo "Boot test failed: Linux directional VMA copies were not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux root working directory validated' "$output_file"; then
    echo "Boot test failed: Linux root working directory was not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux proc self executable link validated' "$output_file"; then
    echo "Boot test failed: Linux proc self executable link was not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux process umask state validated' "$output_file"; then
    echo "Boot test failed: Linux process umask state was not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux saved credential queries validated' "$output_file"; then
    echo "Boot test failed: Linux saved credential queries were not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: Linux immutable resource limits validated' "$output_file"; then
    echo "Boot test failed: Linux immutable resource limits were not validated." >&2
    exit 1
fi

if ! grep -q '^ginux: monotonic sequence exhaustion contained' "$output_file"; then
    echo "Boot test failed: monotonic sequence exhaustion was not contained." >&2
    exit 1
fi

if ! grep -q '^ginux: generation-scoped IPC diagnostics verified' "$output_file"; then
    echo "Boot test failed: IPC diagnostics crossed a service generation." >&2
    exit 1
fi

if [[ "$(grep -c '^USER_OK' "$output_file")" -ne 1 ]]; then
    echo "Boot test failed: the control-plane console record count is invalid." >&2
    exit 1
fi

if ! grep -q '^ginux: opaque capability handles enforced at call gate' "$output_file"; then
    echo "Boot test failed: opaque capability-handle enforcement failed." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary exit status sequenced and retired' "$output_file"; then
    echo "Boot test failed: auxiliary exit status was not sequenced and retired." >&2
    exit 1
fi

if ! grep -q '^ginux: userspace auxiliary creation authority enforced' "$output_file"; then
    echo "Boot test failed: userspace auxiliary creation authority was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: userspace auxiliary stop sequenced and retired' "$output_file"; then
    echo "Boot test failed: userspace auxiliary stop was not sequenced and retired." >&2
    exit 1
fi

if ! grep -q '^ginux: userspace auxiliary load and admission enforced' "$output_file"; then
    echo "Boot test failed: userspace auxiliary load and admission were not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: private auxiliary payload initialization enforced' "$output_file"; then
    echo "Boot test failed: private auxiliary payload initialization was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary initialization state observable' "$output_file"; then
    echo "Boot test failed: auxiliary initialization state was not observable." >&2
    exit 1
fi

if ! grep -q '^ginux: uninitialized auxiliary admission denied' "$output_file"; then
    echo "Boot test failed: uninitialized auxiliary admission was not denied." >&2
    exit 1
fi

if ! grep -q '^ginux: explicit auxiliary start authority enforced' "$output_file"; then
    echo "Boot test failed: explicit auxiliary start authority was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary runtime state query enforced' "$output_file"; then
    echo "Boot test failed: auxiliary runtime state queries were not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: userspace auxiliary lifecycle observation enforced' "$output_file"; then
    echo "Boot test failed: userspace auxiliary lifecycle observation was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: userspace auxiliary lifecycle descriptor enforced' "$output_file"; then
    echo "Boot test failed: userspace auxiliary lifecycle descriptor validation was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: userspace auxiliary service identity enforced' "$output_file"; then
    echo "Boot test failed: userspace auxiliary service identity validation was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: auxiliary management authority denied' "$output_file"; then
    echo "Boot test failed: auxiliary management authority was not denied." >&2
    exit 1
fi

if ! grep -q '^ginux: isolated network device probe completed' "$output_file"; then
    echo "Boot test failed: isolated network device probe did not complete." >&2
    exit 1
fi

if ! grep -q '^ginux: network device command validation enforced' "$output_file"; then
    echo "Boot test failed: network device command validation was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: network link state configured' "$output_file"; then
    echo "Boot test failed: network link state was not configured and observed." >&2
    exit 1
fi

if ! grep -q '^ginux: network IPv4 configuration retained' "$output_file"; then
    echo "Boot test failed: network IPv4 configuration was not retained." >&2
    exit 1
fi

if ! grep -q '^ginux: network default gateway validated' "$output_file"; then
    echo "Boot test failed: network default gateway validation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: network configuration generation tracked' "$output_file"; then
    echo "Boot test failed: network configuration generation was not tracked." >&2
    exit 1
fi

if ! grep -q '^ginux: network gateway invalidated on address change' "$output_file"; then
    echo "Boot test failed: stale network gateway survived an address change." >&2
    exit 1
fi

if ! grep -q '^ginux: bounded static network route retained' "$output_file"; then
    echo "Boot test failed: bounded static network route was not retained." >&2
    exit 1
fi

if ! grep -q '^ginux: coherent network state snapshot copied' "$output_file"; then
    echo "Boot test failed: coherent network state snapshot was not copied." >&2
    exit 1
fi

if ! grep -q '^ginux: network device capability revoked on exit' "$output_file"; then
    echo "Boot test failed: network device authority survived service exit." >&2
    exit 1
fi

if ! grep -q '^ginux: read-only network observer isolated' "$output_file"; then
    echo "Boot test failed: read-only network observer was not isolated." >&2
    exit 1
fi

if ! grep -q '^ginux: network mutation denied to observer' "$output_file"; then
    echo "Boot test failed: network observer mutation was not denied." >&2
    exit 1
fi

if ! grep -q '^ginux: observer capability revoked on exit' "$output_file"; then
    echo "Boot test failed: observer network capability survived exit." >&2
    exit 1
fi

if ! grep -q '^ginux: write-only network configurator isolated' "$output_file"; then
    echo "Boot test failed: write-only network configurator was not isolated." >&2
    exit 1
fi

if ! grep -q '^ginux: network observation denied to configurator' "$output_file"; then
    echo "Boot test failed: network configurator observation was not denied." >&2
    exit 1
fi

if ! grep -q '^ginux: denied network copy-out left memory unchanged' "$output_file"; then
    echo "Boot test failed: denied network copy-out changed userspace memory." >&2
    exit 1
fi

if ! grep -q '^ginux: stale network configuration apply rejected' "$output_file"; then
    echo "Boot test failed: stale network configuration apply was not rejected." >&2
    exit 1
fi

if ! grep -q '^ginux: atomic network configuration apply completed' "$output_file"; then
    echo "Boot test failed: atomic network configuration apply did not complete." >&2
    exit 1
fi

if ! grep -q '^ginux: configurator capability revoked on exit' "$output_file"; then
    echo "Boot test failed: configurator network capability survived exit." >&2
    exit 1
fi

if ! grep -q '^ginux: faulted network service contained' "$output_file"; then
    echo "Boot test failed: faulted network service was not contained." >&2
    exit 1
fi

if ! grep -q '^ginux: faulted network authority revoked' "$output_file"; then
    echo "Boot test failed: faulted network service retained authority." >&2
    exit 1
fi

if ! grep -q '^ginux: network state survived service fault' "$output_file"; then
    echo "Boot test failed: network state did not survive a service fault." >&2
    exit 1
fi

if ! grep -q '^ginux: replacement network service observed retained state' "$output_file"; then
    echo "Boot test failed: replacement network service did not recover retained state." >&2
    exit 1
fi

if ! grep -q '^ginux: replacement network capability lifecycle isolated' "$output_file"; then
    echo "Boot test failed: replacement network capability lifecycle was not isolated." >&2
    exit 1
fi

if ! grep -q '^ginux: failed auxiliary load rollback enforced' "$output_file"; then
    echo "Boot test failed: failed auxiliary load rollback was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: queued auxiliary cancellation and teardown enforced' "$output_file"; then
    echo "Boot test failed: queued auxiliary cancellation was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: ready auxiliary cancellation and teardown enforced' "$output_file"; then
    echo "Boot test failed: ready auxiliary cancellation was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: yielded auxiliary cancellation and teardown enforced' "$output_file"; then
    echo "Boot test failed: yielded auxiliary cancellation was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: stop-only auxiliary fault executed and contained' "$output_file"; then
    echo "Boot test failed: stop-only auxiliary fault was not contained." >&2
    exit 1
fi

if ! grep -q '^ginux: control call ABI v2 negotiated' "$output_file"; then
    echo "Boot test failed: control call ABI negotiation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: userspace console capability enforced' "$output_file"; then
    echo "Boot test failed: userspace console authority failed." >&2
    exit 1
fi

if ! grep -q '^ginux: minimal user interrupt surface enforced' "$output_file"; then
    echo "Boot test failed: unexpected interrupt gates are callable from userspace." >&2
    exit 1
fi

if ! grep -q '^ginux: bounded userspace read enforced' "$output_file"; then
    echo "Boot test failed: bounded userspace copying failed." >&2
    exit 1
fi

if ! grep -q '^ginux: user copies bound to owned mappings' "$output_file"; then
    echo "Boot test failed: user copies were not bound to owned mappings." >&2
    exit 1
fi

if ! grep -q '^ginux: page-granular user range validation enforced' "$output_file"; then
    echo "Boot test failed: page-granular user range validation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: structured userspace ABI result verified' "$output_file"; then
    echo "Boot test failed: bounded userspace result copying failed." >&2
    exit 1
fi

if ! grep -q '^ginux: ring-3 fault survivor resumed and exited' "$output_file"; then
    echo "Boot test failed: the surviving ring-3 context did not complete." >&2
    exit 1
fi

if ! grep -q '^ginux: user page fault contained' "$output_file"; then
    echo "Boot test failed: a ring-3 fault was not contained." >&2
    exit 1
fi

if ! grep -q '^ginux: isolated scheduler address spaces switched' "$output_file"; then
    echo "Boot test failed: scheduler address-space switching failed." >&2
    exit 1
fi

if ! grep -q '^ginux: scheduler CR3 roots validated' "$output_file"; then
    echo "Boot test failed: scheduler CR3 validation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: bounded user page-table hierarchy validated' "$output_file"; then
    echo "Boot test failed: user page-table hierarchy validation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: address-space resource contract shared' "$output_file"; then
    echo "Boot test failed: address-space resource contract was not shared." >&2
    exit 1
fi

if ! grep -q '^ginux: scheduler address-space ownership validated' "$output_file"; then
    echo "Boot test failed: scheduler address-space ownership validation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: scheduler user mappings validated' "$output_file"; then
    echo "Boot test failed: scheduler user mapping validation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: one-shot user preemption completed' "$output_file"; then
    echo "Boot test failed: the one-shot user preemption did not complete." >&2
    exit 1
fi

if ! grep -q '^ginux: restarted service preemption completed' "$output_file"; then
    echo "Boot test failed: the restarted service was not reached by preemption." >&2
    exit 1
fi

if ! grep -q '^ginux: completed address-space frames reclaimed' "$output_file"; then
    echo "Boot test failed: completed address-space frames were not reclaimed." >&2
    exit 1
fi

if ! grep -q '^ginux: retired domain frame ownership cleared' "$output_file"; then
    echo "Boot test failed: retired domains retained physical-frame ownership." >&2
    exit 1
fi

if ! grep -q '^ginux: retired service frame quarantine cleared' "$output_file"; then
    echo "Boot test failed: retired service frame quarantine remained live." >&2
    exit 1
fi

if ! grep -q '^ginux: capability-gated bounded IPC verified' "$output_file"; then
    echo "Boot test failed: bounded IPC authority or FIFO behavior failed." >&2
    exit 1
fi

if ! grep -q '^ginux: directional IPC endpoints isolated' "$output_file"; then
    echo "Boot test failed: directional IPC endpoint isolation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: generation-bound IPC endpoints verified' "$output_file"; then
    echo "Boot test failed: IPC endpoint generations were not isolated." >&2
    exit 1
fi

if ! grep -q '^ginux: generation-tagged IPC messages verified' "$output_file"; then
    echo "Boot test failed: IPC message generations were not retained." >&2
    exit 1
fi

if ! grep -q '^ginux: monotonic queued IPC sequences verified' "$output_file"; then
    echo "Boot test failed: queued IPC sequence validation failed." >&2
    exit 1
fi

if ! grep -q '^ginux: structured IPC delivery provenance verified' "$output_file"; then
    echo "Boot test failed: structured IPC provenance was not verified." >&2
    exit 1
fi

if ! grep -q '^ginux: IPC diagnostic authority enforced' "$output_file"; then
    echo "Boot test failed: IPC diagnostic authority was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: blocking IPC direct wake completed' "$output_file"; then
    echo "Boot test failed: blocking IPC did not wake the waiting service." >&2
    exit 1
fi

if ! grep -q '^ginux: direct IPC wake provenance verified' "$output_file"; then
    echo "Boot test failed: direct IPC wake provenance was not retained." >&2
    exit 1
fi

if ! grep -q '^ginux: sequenced direct IPC delivery verified' "$output_file"; then
    echo "Boot test failed: direct IPC delivery was not sequenced." >&2
    exit 1
fi

if ! grep -q '^ginux: IPC wait-key matching enforced' "$output_file"; then
    echo "Boot test failed: IPC wait-key matching was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: blocked IPC peer-close cleanup verified' "$output_file"; then
    echo "Boot test failed: blocked IPC peer-close cleanup failed." >&2
    exit 1
fi

if ! grep -q '^ginux: sticky IPC peer closure delivered' "$output_file"; then
    echo "Boot test failed: a late IPC receiver did not observe peer closure." >&2
    exit 1
fi

if ! grep -q '^ginux: monotonic IPC peer closures verified' "$output_file"; then
    echo "Boot test failed: IPC peer closures were not sequenced." >&2
    exit 1
fi

if ! grep -q '^ginux: isolated service request-reply completed' "$output_file"; then
    echo "Boot test failed: isolated service request/reply did not complete." >&2
    exit 1
fi

if ! grep -q '^ginux: service domain authority restricted to IPC' "$output_file"; then
    echo "Boot test failed: service-domain least authority was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: faulted service domain authority retired' "$output_file"; then
    echo "Boot test failed: faulted service-domain authority remained live." >&2
    exit 1
fi

if ! grep -q '^ginux: service fault event delivered to control plane' "$output_file"; then
    echo "Boot test failed: the service fault event did not reach the control plane." >&2
    exit 1
fi

if ! grep -q '^ginux: service fault event generation matched' "$output_file"; then
    echo "Boot test failed: service fault event generation did not match." >&2
    exit 1
fi

if ! grep -q '^ginux: monotonic lifecycle events verified' "$output_file"; then
    echo "Boot test failed: lifecycle events were not sequenced." >&2
    exit 1
fi

if ! grep -q '^ginux: structured lifecycle event delivered' "$output_file"; then
    echo "Boot test failed: structured lifecycle event was not delivered." >&2
    exit 1
fi

if ! grep -q '^ginux: structured lifecycle service identity delivered' "$output_file"; then
    echo "Boot test failed: lifecycle service identity was not delivered." >&2
    exit 1
fi

if ! grep -q '^ginux: lifecycle diagnostic authority enforced' "$output_file"; then
    echo "Boot test failed: lifecycle diagnostic authority was not enforced." >&2
    exit 1
fi

if ! grep -q '^ginux: service fault metadata matched' "$output_file"; then
    echo "Boot test failed: service fault metadata did not match." >&2
    exit 1
fi

if ! grep -q '^ginux: structured service fault delivered' "$output_file"; then
    echo "Boot test failed: structured service fault delivery failed." >&2
    exit 1
fi

if ! grep -q '^ginux: service fault diagnostic authority enforced' "$output_file"; then
    echo "Boot test failed: service fault diagnostic authority failed." >&2
    exit 1
fi

if ! grep -q '^ginux: service domain generation reuse verified' "$output_file"; then
    echo "Boot test failed: service-domain slot reuse accepted stale identity." >&2
    exit 1
fi

if ! grep -q '^ginux: stable service identity survived restart' "$output_file"; then
    echo "Boot test failed: stable service identity did not survive restart." >&2
    exit 1
fi

if ! grep -q '^ginux: service restart budget exhausted' "$output_file"; then
    echo "Boot test failed: service restart budget was not exhausted." >&2
    exit 1
fi

if ! grep -q '^ginux: service restart rollback invariants verified' "$output_file"; then
    echo "Boot test failed: service restart rollback invariants failed." >&2
    exit 1
fi

if ! grep -q '^ginux: failed service stack scrubbed before restart' "$output_file"; then
    echo "Boot test failed: failed service stack was not scrubbed." >&2
    exit 1
fi

if ! grep -q '^ginux: service frame ownership transferred to replacement' "$output_file"; then
    echo "Boot test failed: service frame ownership was not transferred." >&2
    exit 1
fi

if ! grep -q '^ginux: replacement service context executed' "$output_file"; then
    echo "Boot test failed: replacement service context did not execute." >&2
    exit 1
fi

if ! grep -q '^ginux: replacement service request-reply completed' "$output_file"; then
    echo "Boot test failed: replacement service request-reply did not complete." >&2
    exit 1
fi

if ! grep -q '^ginux: volatile storage service round-trip completed' "$output_file"; then
    echo "Boot test failed: volatile storage service round-trip did not complete." >&2
    exit 1
fi

if ! grep -q '^ginux: volatile storage invalid command rejected' "$output_file"; then
    echo "Boot test failed: volatile storage invalid command was not rejected." >&2
    exit 1
fi

if ! grep -q '^ginux: replacement service readiness delivered' "$output_file"; then
    echo "Boot test failed: replacement service readiness was not delivered." >&2
    exit 1
fi

if ! grep -q '^ginux: replacement service clean exit delivered' "$output_file"; then
    echo "Boot test failed: replacement service clean exit was not delivered." >&2
    exit 1
fi

if ! grep -q '^ginux: per-domain syscall budgets enforced' "$output_file"; then
    echo "Boot test failed: per-domain syscall accounting failed." >&2
    exit 1
fi

if ! grep -q '^ginux: per-domain thread budgets enforced' "$output_file"; then
    echo "Boot test failed: per-domain thread accounting failed." >&2
    exit 1
fi

if ! grep -q '^ginux: in-flight domain operations drained' "$output_file"; then
    echo "Boot test failed: in-flight domain operation accounting failed." >&2
    exit 1
fi

if ! grep -q '^ginux: scheduler context lifecycle verified' "$output_file"; then
    echo "Boot test failed: scheduler context lifecycle invariants failed." >&2
    exit 1
fi

if ! grep -q '^ginux: preemptible user context flags enforced' "$output_file"; then
    echo "Boot test failed: scheduler accepted a non-preemptible user context." >&2
    exit 1
fi

if ! grep -q '^ginux: bootstrap SHA-256 known answers verified' "$output_file"; then
    echo "Boot test failed: bootstrap SHA-256 known-answer tests failed." >&2
    exit 1
fi

if grep -qE '^GINUX_(BOOT|KERNEL)_PANIC' "$output_file"; then
    echo "Boot test failed: a panic marker was emitted." >&2
    exit 1
fi

if [[ "$qemu_status" -ne 0 ]]; then
    echo "Boot test failed: QEMU exited with $qemu_status; expected 0." >&2
    exit 1
fi

echo "Boot test passed."

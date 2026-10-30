# Gate 3 userspace-access audit

Gate 3 makes hardware page-table faults authoritative for kernel copies to and
from untrusted userspace addresses. The x86-64 copy loops open SMAP only for a
bounded assembly region. A page fault in that region redirects execution to a
local fixup that closes SMAP and returns the number of bytes not copied.

## Access boundary

- Control-domain callers use `copy_from_current_user` and
  `copy_to_current_user`, or their partial-copy variants.
- Linux syscall callers use `copy_from_linux_user` and `copy_to_linux_user`.
  Software range metadata still supplies policy and COW preparation, but the
  copy succeeds only when the hardware access succeeds.
- The raw copy functions are used directly only by their adversarial runtime
  test and by kernel-owned verification paths that intentionally bypass a
  higher-level policy check.
- `with_supervisor_user_access` is no longer called by syscall implementation
  code. Its remaining calls in `address_space.rs` initialize or verify mappings
  created and exclusively owned by the kernel while the corresponding page
  table root is active; they are not accesses to syscall-supplied pointers.

`tools/architecture/check-user-access.sh` enforces the presence of the fault
regions and fixups, the public partial-copy API, the absence of direct
SMAP-opening calls in `cpu.rs`, and the absence of direct scalar dereferences
through cast userspace pointers there.

## Negative behavior

The boot-time adversarial test crosses from a mapped page into a guard page in
both directions, then starts both copies on a wholly unmapped page. It requires
the partial and full remaining-byte counts and verifies that the AC flag is
clear after every path. Existing ABI probes exercise noncanonical, kernel,
read-only, guard-page, and invalid syscall pointers and require `EFAULT` or the
interface-specific defined error instead of a kernel panic.

The deliberately unmapped hardware fault after entering the copy loop is also
the mapping-change proof: recovery does not depend on the earlier software
range decision remaining true.

use core::cell::UnsafeCell;
use core::cmp;
#[cfg(not(test))]
use core::ffi::c_char;
use core::ffi::{c_int, c_void};
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering};

const MAX_ALLOCATIONS: usize = 16_384;
const DEFAULT_ALIGNMENT: usize = 16;
const MAX_ALIGNMENT: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy)]
struct Slot {
    user: usize,
    mapping: usize,
    mapping_len: usize,
    requested: usize,
    alignment: usize,
    generation: u64,
    live: bool,
}

impl Slot {
    const EMPTY: Self = Self {
        user: 0,
        mapping: 0,
        mapping_len: 0,
        requested: 0,
        alignment: 0,
        generation: 0,
        live: false,
    };
}

struct Registry(UnsafeCell<[Slot; MAX_ALLOCATIONS]>);

// SAFETY: every access to the contained slots is serialized by REGISTRY_LOCK.
unsafe impl Sync for Registry {}

static REGISTRY: Registry = Registry(UnsafeCell::new([Slot::EMPTY; MAX_ALLOCATIONS]));
static REGISTRY_LOCK: AtomicBool = AtomicBool::new(false);
static PROCESS_ID: AtomicI32 = AtomicI32::new(0);
static PAGE_SIZE: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(1);
static RANDOM_FALLBACK: AtomicU64 = AtomicU64::new(0x9e37_79b9_7f4a_7c15);

struct RegistryGuard;

impl Drop for RegistryGuard {
    fn drop(&mut self) {
        REGISTRY_LOCK.store(false, Ordering::Release);
    }
}

fn current_pid() -> i32 {
    // SAFETY: getpid has no pointer arguments and cannot violate memory safety.
    unsafe { libc::getpid() }
}

fn lock_registry() -> RegistryGuard {
    let pid = current_pid();
    let recorded = PROCESS_ID.load(Ordering::Acquire);
    if recorded != pid {
        // A child may inherit a lock held by a vanished sibling thread. The PID
        // transition makes that inherited ownership impossible, so reset it.
        PROCESS_ID.store(pid, Ordering::Release);
        REGISTRY_LOCK.store(false, Ordering::Release);
    }
    while REGISTRY_LOCK
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }
    RegistryGuard
}

fn page_size() -> usize {
    let cached = PAGE_SIZE.load(Ordering::Acquire);
    if cached != 0 {
        return cached;
    }
    // SAFETY: sysconf has no pointer arguments. An invalid result is replaced
    // with the Linux minimum page size used by supported Ginux hosts.
    let probed = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    let value = if probed >= 4096 {
        probed as usize
    } else {
        4096
    };
    PAGE_SIZE.store(value, Ordering::Release);
    value
}

fn align_up(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment.checked_sub(1)?)
        .map(|sum| sum & !(alignment - 1))
}

fn valid_alignment(alignment: usize) -> bool {
    alignment >= core::mem::size_of::<usize>()
        && alignment <= MAX_ALIGNMENT
        && alignment.is_power_of_two()
}

fn random_word() -> u64 {
    #[cfg(target_os = "linux")]
    {
        let mut value = 0u64;
        // SAFETY: the destination is a valid local u64 for the supplied size.
        let result = unsafe {
            libc::syscall(
                libc::SYS_getrandom,
                &mut value as *mut u64,
                core::mem::size_of::<u64>(),
                0,
            )
        };
        if result == core::mem::size_of::<u64>() as libc::c_long {
            return value;
        }
    }
    let old = RANDOM_FALLBACK.fetch_add(0xa076_1d64_78bd_642f, Ordering::Relaxed);
    old ^ old.rotate_left(23)
}

#[cfg(target_os = "linux")]
fn set_errno(value: c_int) {
    // SAFETY: __errno_location returns the calling thread's valid errno slot.
    unsafe { *libc::__errno_location() = value };
}

#[cfg(not(target_os = "linux"))]
fn set_errno(_value: c_int) {}

fn abort_invalid_free() -> ! {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: exit_group terminates the current process and has no pointer
        // arguments. It deliberately avoids allocator-dependent libc cleanup.
        unsafe { libc::syscall(libc::SYS_exit_group, 134) };
    }
    // SAFETY: the Linux syscall above does not return on supported production
    // targets. Other targets use abort solely for developer tests.
    unsafe { libc::abort() }
}

#[cfg(target_os = "linux")]
fn parse_decimal(bytes: &[u8]) -> Option<usize> {
    if bytes.is_empty() {
        return None;
    }
    let mut value = 0usize;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add((byte - b'0') as usize)?;
    }
    Some(value)
}

#[cfg(target_os = "linux")]
fn read_small_file(path: &[u8]) -> Option<usize> {
    let mut buffer = [0u8; 64];
    // SAFETY: path is a static NUL-terminated byte string.
    let fd = unsafe {
        libc::open(
            path.as_ptr().cast::<c_char>(),
            libc::O_RDONLY | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return None;
    }
    // SAFETY: buffer is valid and writable for its complete length.
    let length = unsafe { libc::read(fd, buffer.as_mut_ptr().cast::<c_void>(), buffer.len()) };
    // SAFETY: fd was returned by open and is no longer used after this call.
    unsafe { libc::close(fd) };
    if length <= 0 {
        return None;
    }
    let end = buffer[..length as usize]
        .iter()
        .position(|byte| byte.is_ascii_whitespace())
        .unwrap_or(length as usize);
    parse_decimal(&buffer[..end])
}

#[cfg(not(target_os = "linux"))]
fn read_small_file(_path: &[u8]) -> Option<usize> {
    None
}

fn instance_memory_limit() -> usize {
    read_small_file(b"/sys/fs/cgroup/memory.max\0").unwrap_or(usize::MAX)
}

fn reserve_live_bytes(size: usize) -> bool {
    let live = LIVE_BYTES.load(Ordering::Relaxed);
    match live.checked_add(size) {
        Some(next) if next <= instance_memory_limit() => {
            LIVE_BYTES.store(next, Ordering::Relaxed);
            true
        }
        _ => false,
    }
}

fn allocate(size: usize, alignment: usize, zero: bool) -> *mut c_void {
    if !valid_alignment(alignment) {
        set_errno(libc::EINVAL);
        return ptr::null_mut();
    }
    let requested = cmp::max(size, 1);
    let page = page_size();
    let slack = match alignment.checked_sub(1).and_then(|v| v.checked_add(page)) {
        Some(value) => value,
        None => {
            set_errno(libc::ENOMEM);
            return ptr::null_mut();
        }
    };
    let data_len = match requested
        .checked_add(slack)
        .and_then(|value| align_up(value, page))
    {
        Some(value) => value,
        None => {
            set_errno(libc::ENOMEM);
            return ptr::null_mut();
        }
    };
    let mapping_len = match data_len.checked_add(page.saturating_mul(2)) {
        Some(value) => value,
        None => {
            set_errno(libc::ENOMEM);
            return ptr::null_mut();
        }
    };

    let _guard = lock_registry();
    // SAFETY: registry access is serialized by REGISTRY_LOCK.
    let slots = unsafe { &mut *REGISTRY.0.get() };
    let Some(slot) = slots.iter_mut().find(|slot| !slot.live) else {
        set_errno(libc::ENOMEM);
        return ptr::null_mut();
    };
    if !reserve_live_bytes(requested) {
        set_errno(libc::ENOMEM);
        return ptr::null_mut();
    }

    // SAFETY: mmap is called with no input pointer and a checked length.
    let mapping = unsafe {
        libc::mmap(
            ptr::null_mut(),
            mapping_len,
            libc::PROT_NONE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if mapping == libc::MAP_FAILED {
        LIVE_BYTES.fetch_sub(requested, Ordering::Relaxed);
        set_errno(libc::ENOMEM);
        return ptr::null_mut();
    }
    let data = mapping as usize + page;
    let offset = (random_word() as usize % page) & !(DEFAULT_ALIGNMENT - 1);
    let Some(user) = align_up(data + offset, alignment) else {
        // SAFETY: mapping and length came from the successful mmap above.
        unsafe { libc::munmap(mapping, mapping_len) };
        LIVE_BYTES.fetch_sub(requested, Ordering::Relaxed);
        set_errno(libc::ENOMEM);
        return ptr::null_mut();
    };
    if user
        .checked_add(requested)
        .is_none_or(|end| end > data + data_len)
    {
        // SAFETY: mapping and length came from the successful mmap above.
        unsafe { libc::munmap(mapping, mapping_len) };
        LIVE_BYTES.fetch_sub(requested, Ordering::Relaxed);
        set_errno(libc::ENOMEM);
        return ptr::null_mut();
    }
    // SAFETY: data identifies the complete interior page range of the mapping.
    if unsafe {
        libc::mprotect(
            data as *mut c_void,
            data_len,
            libc::PROT_READ | libc::PROT_WRITE,
        )
    } != 0
    {
        // SAFETY: mapping and length came from the successful mmap above.
        unsafe { libc::munmap(mapping, mapping_len) };
        LIVE_BYTES.fetch_sub(requested, Ordering::Relaxed);
        set_errno(libc::ENOMEM);
        return ptr::null_mut();
    }
    if zero {
        // SAFETY: user..user+requested is inside the writable interior mapping.
        unsafe { ptr::write_bytes(user as *mut u8, 0, requested) };
    }
    *slot = Slot {
        user,
        mapping: mapping as usize,
        mapping_len,
        requested,
        alignment,
        generation: GENERATION.fetch_add(1, Ordering::Relaxed),
        live: true,
    };
    LIVE_ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    user as *mut c_void
}

fn allocation_size(pointer: *mut c_void) -> Option<usize> {
    if pointer.is_null() {
        return Some(0);
    }
    let _guard = lock_registry();
    // SAFETY: registry access is serialized by REGISTRY_LOCK.
    let slots = unsafe { &*REGISTRY.0.get() };
    slots
        .iter()
        .find(|slot| slot.live && slot.user == pointer as usize)
        .map(|slot| slot.requested)
}

fn deallocate(pointer: *mut c_void) {
    if pointer.is_null() {
        return;
    }
    let (mapping, mapping_len, requested) = {
        let _guard = lock_registry();
        // SAFETY: registry access is serialized by REGISTRY_LOCK.
        let slots = unsafe { &mut *REGISTRY.0.get() };
        let Some(slot) = slots.iter_mut().find(|slot| slot.user == pointer as usize) else {
            abort_invalid_free();
        };
        if !slot.live || slot.generation == 0 {
            abort_invalid_free();
        }
        slot.live = false;
        LIVE_BYTES.fetch_sub(slot.requested, Ordering::Relaxed);
        LIVE_ALLOCATIONS.fetch_sub(1, Ordering::Relaxed);
        (slot.mapping, slot.mapping_len, slot.requested)
    };
    // SAFETY: the mapping belongs exclusively to this live allocation and was
    // removed from the registry before being unmapped.
    if unsafe { libc::munmap(mapping as *mut c_void, mapping_len) } != 0 {
        let _ = requested;
        abort_invalid_free();
    }
}

fn reallocate(pointer: *mut c_void, size: usize) -> *mut c_void {
    if pointer.is_null() {
        return allocate(size, DEFAULT_ALIGNMENT, false);
    }
    if size == 0 {
        deallocate(pointer);
        return ptr::null_mut();
    }
    let (old_size, alignment) = {
        let _guard = lock_registry();
        // SAFETY: registry access is serialized by REGISTRY_LOCK.
        let slots = unsafe { &*REGISTRY.0.get() };
        let Some(slot) = slots
            .iter()
            .find(|slot| slot.live && slot.user == pointer as usize)
        else {
            abort_invalid_free();
        };
        (slot.requested, slot.alignment)
    };
    let replacement = allocate(size, alignment, false);
    if replacement.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: both allocations are live, non-overlapping, and valid for the
    // minimum of their requested sizes.
    unsafe {
        ptr::copy_nonoverlapping(
            pointer.cast::<u8>(),
            replacement.cast::<u8>(),
            cmp::min(old_size, size),
        )
    };
    deallocate(pointer);
    replacement
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn malloc(size: usize) -> *mut c_void {
    allocate(size, DEFAULT_ALIGNMENT, false)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn free(pointer: *mut c_void) {
    deallocate(pointer)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn calloc(count: usize, size: usize) -> *mut c_void {
    let Some(total) = count.checked_mul(size) else {
        set_errno(libc::ENOMEM);
        return ptr::null_mut();
    };
    allocate(total, DEFAULT_ALIGNMENT, true)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
    reallocate(pointer, size)
}

#[cfg(not(test))]
#[no_mangle]
/// Store an aligned allocation in the caller-provided output pointer.
///
/// # Safety
///
/// `output` must be writable for one pointer, as required by POSIX.
pub unsafe extern "C" fn posix_memalign(
    output: *mut *mut c_void,
    alignment: usize,
    size: usize,
) -> c_int {
    if output.is_null() || !valid_alignment(alignment) {
        return libc::EINVAL;
    }
    let allocation = allocate(size, alignment, false);
    if allocation.is_null() {
        return libc::ENOMEM;
    }
    // SAFETY: the caller provided a non-null output pointer as required by the
    // posix_memalign ABI.
    unsafe { output.write(allocation) };
    0
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn aligned_alloc(alignment: usize, size: usize) -> *mut c_void {
    if !size.is_multiple_of(alignment.max(1)) {
        set_errno(libc::EINVAL);
        return ptr::null_mut();
    }
    allocate(size, alignment, false)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn memalign(alignment: usize, size: usize) -> *mut c_void {
    allocate(size, alignment, false)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn valloc(size: usize) -> *mut c_void {
    allocate(size, page_size(), false)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn pvalloc(size: usize) -> *mut c_void {
    let Some(rounded) = align_up(cmp::max(size, 1), page_size()) else {
        set_errno(libc::ENOMEM);
        return ptr::null_mut();
    };
    allocate(rounded, page_size(), false)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn reallocarray(pointer: *mut c_void, count: usize, size: usize) -> *mut c_void {
    let Some(total) = count.checked_mul(size) else {
        set_errno(libc::ENOMEM);
        return ptr::null_mut();
    };
    reallocate(pointer, total)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn malloc_usable_size(pointer: *mut c_void) -> usize {
    allocation_size(pointer).unwrap_or_else(|| abort_invalid_free())
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn cfree(pointer: *mut c_void) {
    deallocate(pointer)
}

fn mallocx_alignment(flags: c_int) -> usize {
    let lg = (flags as usize) & 0x3f;
    if lg == 0 {
        DEFAULT_ALIGNMENT
    } else {
        1usize.checked_shl(lg as u32).unwrap_or(0)
    }
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn mallocx(size: usize, flags: c_int) -> *mut c_void {
    allocate(size, mallocx_alignment(flags), flags & 0x40 != 0)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn rallocx(pointer: *mut c_void, size: usize, _flags: c_int) -> *mut c_void {
    reallocate(pointer, size)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn xallocx(
    pointer: *mut c_void,
    size: usize,
    _extra: usize,
    _flags: c_int,
) -> usize {
    match allocation_size(pointer) {
        Some(current) if size <= current => current,
        Some(current) => current,
        None => abort_invalid_free(),
    }
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn sallocx(pointer: *mut c_void, _flags: c_int) -> usize {
    malloc_usable_size(pointer)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn dallocx(pointer: *mut c_void, _flags: c_int) {
    deallocate(pointer)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn sdallocx(pointer: *mut c_void, _size: usize, _flags: c_int) {
    deallocate(pointer)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn nallocx(size: usize, flags: c_int) -> usize {
    let alignment = mallocx_alignment(flags);
    if valid_alignment(alignment) {
        align_up(cmp::max(size, 1), alignment).unwrap_or(0)
    } else {
        0
    }
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn mallctl(
    _name: *const c_char,
    _old: *mut c_void,
    _old_len: *mut usize,
    _new: *mut c_void,
    _new_len: usize,
) -> c_int {
    libc::ENOENT
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn mallctlnametomib(
    _name: *const c_char,
    _mib: *mut usize,
    _mib_len: *mut usize,
) -> c_int {
    libc::ENOENT
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn mallctlbymib(
    _mib: *const usize,
    _mib_len: usize,
    _old: *mut c_void,
    _old_len: *mut usize,
    _new: *mut c_void,
    _new_len: usize,
) -> c_int {
    libc::ENOENT
}

#[cfg(not(test))]
#[no_mangle]
/// Write the allocator summary through a jemalloc-compatible callback.
///
/// # Safety
///
/// The callback and opaque context must satisfy the jemalloc callback ABI.
pub unsafe extern "C" fn malloc_stats_print(
    callback: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
    opaque: *mut c_void,
    _options: *const c_char,
) {
    static MESSAGE: &[u8] = b"ginux-allocator: guarded mmap allocator\n\0";
    if let Some(write) = callback {
        // SAFETY: MESSAGE is static and NUL terminated as required by the API.
        unsafe { write(opaque, MESSAGE.as_ptr().cast::<c_char>()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_is_aligned_guarded_and_tracked() {
        let pointer = allocate(123, 64, false);
        assert!(!pointer.is_null());
        assert_eq!(pointer as usize % 64, 0);
        assert_eq!(allocation_size(pointer), Some(123));
        assert_eq!(LIVE_ALLOCATIONS.load(Ordering::Relaxed), 1);
        deallocate(pointer);
        assert_eq!(LIVE_ALLOCATIONS.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn calloc_and_reallocate_preserve_data() {
        let pointer = allocate(32, DEFAULT_ALIGNMENT, true);
        assert!(!pointer.is_null());
        // SAFETY: pointer refers to a live 32-byte allocation.
        let bytes = unsafe { core::slice::from_raw_parts_mut(pointer.cast::<u8>(), 32) };
        assert!(bytes.iter().all(|byte| *byte == 0));
        bytes.copy_from_slice(&[0x5a; 32]);
        let replacement = reallocate(pointer, 64);
        assert!(!replacement.is_null());
        // SAFETY: replacement refers to a live 64-byte allocation.
        let copied = unsafe { core::slice::from_raw_parts(replacement.cast::<u8>(), 32) };
        assert!(copied.iter().all(|byte| *byte == 0x5a));
        deallocate(replacement);
    }

    #[test]
    fn arithmetic_and_alignment_fail_closed() {
        assert!(allocate(8, 3, false).is_null());
        assert!(allocate(usize::MAX, DEFAULT_ALIGNMENT, false).is_null());
        assert_eq!(align_up(17, 16), Some(32));
        assert_eq!(nallocx_for_test(17, 4), 32);
        assert_eq!(mallocx_alignment(4), 16);
    }

    fn nallocx_for_test(size: usize, lg_alignment: usize) -> usize {
        let alignment = 1usize << lg_alignment;
        align_up(cmp::max(size, 1), alignment).unwrap_or(0)
    }
}

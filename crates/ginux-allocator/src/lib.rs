use core::cell::UnsafeCell;
use core::cmp;
#[cfg(any(not(test), target_os = "linux"))]
use core::ffi::c_char;
use core::ffi::{c_int, c_void};
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering};

const MAX_ALLOCATIONS: usize = 16_384;
const DEFAULT_ALIGNMENT: usize = 16;
const MAX_ALIGNMENT: usize = 2 * 1024 * 1024;
const ARENA_BYTES: usize = 1024 * 1024;
const ARENA_SHARDS: usize = 8;
const BITMAP_WORDS: usize = ARENA_BYTES / 16 / 64;
const DIRECTORY_ENTRIES: usize = 512;
const DIRECTORY_SHIFT: usize = 20;
const SIZE_CLASSES: [usize; 13] = [
    16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16_384, 32_768, 65_536,
];

struct Arena {
    payload: AtomicUsize,
    cursor: AtomicUsize,
    live_bytes: AtomicUsize,
    live_allocations: AtomicUsize,
    reserved_bytes: AtomicUsize,
    live: [AtomicU64; BITMAP_WORDS],
}

impl Arena {
    const fn new() -> Self {
        Self {
            payload: AtomicUsize::new(0),
            cursor: AtomicUsize::new(0),
            live_bytes: AtomicUsize::new(0),
            live_allocations: AtomicUsize::new(0),
            reserved_bytes: AtomicUsize::new(0),
            live: [const { AtomicU64::new(0) }; BITMAP_WORDS],
        }
    }
}

static ARENAS: [Arena; SIZE_CLASSES.len() * ARENA_SHARDS] =
    [const { Arena::new() }; SIZE_CLASSES.len() * ARENA_SHARDS];
static DIRECTORY_KEYS: [AtomicUsize; DIRECTORY_ENTRIES] =
    [const { AtomicUsize::new(0) }; DIRECTORY_ENTRIES];
static DIRECTORY_VALUES: [AtomicUsize; DIRECTORY_ENTRIES] =
    [const { AtomicUsize::new(0) }; DIRECTORY_ENTRIES];

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
static RESERVED_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(1);
static RANDOM_FALLBACK: AtomicU64 = AtomicU64::new(0x9e37_79b9_7f4a_7c15);
static RANDOM_SEEDED: AtomicBool = AtomicBool::new(false);
static INSTANCE_MEMORY_LIMIT: AtomicUsize = AtomicUsize::new(0);

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

/// Pure checked model used by the allocator and bounded defensive fuzzing.
#[doc(hidden)]
pub fn guarded_mapping_geometry(
    size: usize,
    alignment: usize,
    page: usize,
) -> Option<(usize, usize, usize)> {
    if !valid_alignment(alignment) || page < 4096 || !page.is_power_of_two() {
        return None;
    }
    let requested = cmp::max(size, 1);
    let slack = alignment.checked_sub(1)?.checked_add(page)?;
    let data_len = align_up(requested.checked_add(slack)?, page)?;
    let mapping_len = data_len.checked_add(page.checked_mul(2)?)?;
    Some((requested, data_len, mapping_len))
}

fn random_word() -> u64 {
    if !RANDOM_SEEDED.swap(true, Ordering::AcqRel) {
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
                RANDOM_FALLBACK.store(value.max(1), Ordering::Release);
            }
        }
    }
    let mut old = RANDOM_FALLBACK.load(Ordering::Relaxed);
    loop {
        let mut next = old;
        next ^= next << 13;
        next ^= next >> 7;
        next ^= next << 17;
        match RANDOM_FALLBACK.compare_exchange_weak(old, next, Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => return next,
            Err(observed) => old = observed,
        }
    }
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
    let cached = INSTANCE_MEMORY_LIMIT.load(Ordering::Acquire);
    if cached != 0 {
        return cached;
    }
    let limit = read_small_file(b"/sys/fs/cgroup/memory.max\0").unwrap_or(usize::MAX);
    INSTANCE_MEMORY_LIMIT.store(limit.max(1), Ordering::Release);
    limit
}

fn reserve_capacity(size: usize) -> bool {
    let limit = instance_memory_limit();
    let mut current = RESERVED_BYTES.load(Ordering::Acquire);
    loop {
        let Some(next) = bounded_add(current, size, limit) else {
            return false;
        };
        match RESERVED_BYTES.compare_exchange_weak(
            current,
            next,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return true,
            Err(observed) => current = observed,
        }
    }
}

fn bounded_add(current: usize, additional: usize, limit: usize) -> Option<usize> {
    let next = current.checked_add(additional)?;
    (next <= limit).then_some(next)
}

fn memory_pressure_high() -> bool {
    let limit = instance_memory_limit();
    limit != usize::MAX && LIVE_BYTES.load(Ordering::Relaxed) >= limit.saturating_mul(4) / 5
}

fn class_index(size: usize) -> Option<usize> {
    SIZE_CLASSES.iter().position(|class| size <= *class)
}

fn thread_shard() -> usize {
    // SAFETY: pthread_self returns the calling thread's opaque identity and
    // has no pointer arguments or allocation side effects.
    let identity = unsafe { libc::pthread_self() as usize };
    identity.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> (usize::BITS as usize - 3)
}

fn shard_probe(preferred: usize, probe: usize) -> usize {
    (preferred + probe) % ARENA_SHARDS
}

fn initialize_arena(index: usize) -> Option<usize> {
    let arena = &ARENAS[index];
    let existing = arena.payload.load(Ordering::Acquire);
    if existing != 0 {
        return Some(existing);
    }
    let _guard = lock_registry();
    let existing = arena.payload.load(Ordering::Acquire);
    if existing != 0 {
        return Some(existing);
    }
    // A slab arena has exactly one fixed-size payload mapping. Charge that
    // capacity once, while initialization is serialized, rather than deriving
    // reservations from speculative live-slot counts. Failed initialization
    // rolls the complete charge back before another caller may retry.
    if !reserve_capacity(ARENA_BYTES) {
        return None;
    }
    let page = page_size();
    let Some(mapping_len) = page
        .checked_mul(2)
        .and_then(|guards| ARENA_BYTES.checked_add(guards))
    else {
        RESERVED_BYTES.fetch_sub(ARENA_BYTES, Ordering::AcqRel);
        return None;
    };
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
        RESERVED_BYTES.fetch_sub(ARENA_BYTES, Ordering::AcqRel);
        return None;
    }
    let payload = mapping as usize + page;
    // SAFETY: payload is the complete interior of the new guarded mapping.
    if unsafe {
        libc::mprotect(
            payload as *mut c_void,
            ARENA_BYTES,
            libc::PROT_READ | libc::PROT_WRITE,
        )
    } != 0
    {
        // SAFETY: mapping and length came from the successful mmap above.
        unsafe { libc::munmap(mapping, mapping_len) };
        RESERVED_BYTES.fetch_sub(ARENA_BYTES, Ordering::AcqRel);
        return None;
    }
    let first_key = payload >> DIRECTORY_SHIFT;
    let last_key = (payload + ARENA_BYTES - 1) >> DIRECTORY_SHIFT;
    for key in first_key..=last_key {
        let mut position = key.wrapping_mul(0x9e37_79b9) % DIRECTORY_ENTRIES;
        loop {
            if DIRECTORY_KEYS[position].load(Ordering::Relaxed) == 0 {
                DIRECTORY_VALUES[position].store(index + 1, Ordering::Relaxed);
                DIRECTORY_KEYS[position].store(key + 1, Ordering::Release);
                break;
            }
            position = (position + 1) % DIRECTORY_ENTRIES;
        }
    }
    arena.payload.store(payload, Ordering::Release);
    arena.reserved_bytes.store(ARENA_BYTES, Ordering::Release);
    arena
        .cursor
        .store(random_word() as usize, Ordering::Release);
    Some(payload)
}

fn allocate_slab(requested: usize, zero: bool) -> *mut c_void {
    let Some(class_index) = class_index(requested) else {
        return ptr::null_mut();
    };
    let class = SIZE_CLASSES[class_index];
    let preferred = thread_shard();
    for probe in 0..ARENA_SHARDS {
        let shard = shard_probe(preferred, probe);
        let index = class_index * ARENA_SHARDS + shard;
        let Some(payload) = initialize_arena(index) else {
            continue;
        };
        let slots = ARENA_BYTES / class;
        let start = ARENAS[index].cursor.fetch_add(1, Ordering::Relaxed) % slots;
        for step in 0..slots {
            let slot = (start + step) % slots;
            let word = slot / 64;
            let mask = 1u64 << (slot % 64);
            if ARENAS[index].live[word].fetch_or(mask, Ordering::AcqRel) & mask == 0 {
                let user = payload + slot * class;
                ARENAS[index].live_bytes.fetch_add(class, Ordering::AcqRel);
                LIVE_BYTES.fetch_add(class, Ordering::AcqRel);
                if zero {
                    // SAFETY: this thread owns the selected live slot.
                    unsafe { ptr::write_bytes(user as *mut u8, 0, requested) };
                }
                ARENAS[index]
                    .live_allocations
                    .fetch_add(1, Ordering::Relaxed);
                return user as *mut c_void;
            }
        }
    }
    set_errno(libc::ENOMEM);
    ptr::null_mut()
}

fn allocate_guarded(size: usize, alignment: usize, zero: bool) -> *mut c_void {
    if !valid_alignment(alignment) {
        set_errno(libc::EINVAL);
        return ptr::null_mut();
    }
    let page = page_size();
    let (requested, data_len, mapping_len) = match guarded_mapping_geometry(size, alignment, page) {
        Some(geometry) => geometry,
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
    if !reserve_capacity(requested) {
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
        RESERVED_BYTES.fetch_sub(requested, Ordering::Relaxed);
        set_errno(libc::ENOMEM);
        return ptr::null_mut();
    }
    let data = mapping as usize + page;
    let offset = (random_word() as usize % page) & !(DEFAULT_ALIGNMENT - 1);
    let Some(user) = align_up(data + offset, alignment) else {
        // SAFETY: mapping and length came from the successful mmap above.
        unsafe { libc::munmap(mapping, mapping_len) };
        RESERVED_BYTES.fetch_sub(requested, Ordering::Relaxed);
        set_errno(libc::ENOMEM);
        return ptr::null_mut();
    };
    if user
        .checked_add(requested)
        .is_none_or(|end| end > data + data_len)
    {
        // SAFETY: mapping and length came from the successful mmap above.
        unsafe { libc::munmap(mapping, mapping_len) };
        RESERVED_BYTES.fetch_sub(requested, Ordering::Relaxed);
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
        RESERVED_BYTES.fetch_sub(requested, Ordering::Relaxed);
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
    LIVE_BYTES.fetch_add(requested, Ordering::Relaxed);
    LIVE_ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    user as *mut c_void
}

fn allocate(size: usize, alignment: usize, zero: bool) -> *mut c_void {
    if !valid_alignment(alignment) {
        set_errno(libc::EINVAL);
        return ptr::null_mut();
    }
    let requested = cmp::max(size, 1);
    if alignment <= DEFAULT_ALIGNMENT && requested <= *SIZE_CLASSES.last().unwrap() {
        return allocate_slab(requested, zero);
    }
    allocate_guarded(size, alignment, zero)
}

fn slab_slot(pointer: *mut c_void) -> Option<(usize, usize)> {
    let address = pointer as usize;
    let key = address >> DIRECTORY_SHIFT;
    let mut position = key.wrapping_mul(0x9e37_79b9) % DIRECTORY_ENTRIES;
    for _ in 0..DIRECTORY_ENTRIES {
        let stored_key = DIRECTORY_KEYS[position].load(Ordering::Acquire);
        if stored_key == 0 {
            return None;
        }
        if stored_key == key + 1 {
            let value = DIRECTORY_VALUES[position].load(Ordering::Relaxed);
            if value != 0 {
                let index = value - 1;
                let payload = ARENAS[index].payload.load(Ordering::Acquire);
                if address >= payload && address < payload + ARENA_BYTES {
                    let class = SIZE_CLASSES[index / ARENA_SHARDS];
                    let offset = address - payload;
                    if !offset.is_multiple_of(class) {
                        abort_invalid_free();
                    }
                    return Some((index, offset / class));
                }
            }
        }
        position = (position + 1) % DIRECTORY_ENTRIES;
    }
    None
}

fn allocation_size(pointer: *mut c_void) -> Option<usize> {
    if pointer.is_null() {
        return Some(0);
    }
    if let Some((index, slot)) = slab_slot(pointer) {
        let word = slot / 64;
        let mask = 1u64 << (slot % 64);
        return (ARENAS[index].live[word].load(Ordering::Acquire) & mask != 0)
            .then_some(SIZE_CLASSES[index / ARENA_SHARDS]);
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
    if let Some((index, slot)) = slab_slot(pointer) {
        let class = SIZE_CLASSES[index / ARENA_SHARDS];
        let word = slot / 64;
        let mask = 1u64 << (slot % 64);
        if ARENAS[index].live[word].load(Ordering::Acquire) & mask == 0 {
            abort_invalid_free();
        }
        if class >= page_size() && memory_pressure_high() {
            // Keep the slot live until reclamation is complete so another
            // thread cannot reuse it while madvise is discarding its pages.
            // SAFETY: page-sized classes are page aligned and still live.
            unsafe { libc::madvise(pointer, class, libc::MADV_DONTNEED) };
        }
        if ARENAS[index].live[word].fetch_and(!mask, Ordering::AcqRel) & mask == 0 {
            abort_invalid_free();
        }
        ARENAS[index].live_bytes.fetch_sub(class, Ordering::AcqRel);
        LIVE_BYTES.fetch_sub(class, Ordering::AcqRel);
        ARENAS[index]
            .live_allocations
            .fetch_sub(1, Ordering::Relaxed);
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
        RESERVED_BYTES.fetch_sub(slot.requested, Ordering::Relaxed);
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
    let (old_size, alignment) = if let Some((index, slot)) = slab_slot(pointer) {
        let word = slot / 64;
        let mask = 1u64 << (slot % 64);
        if ARENAS[index].live[word].load(Ordering::Acquire) & mask == 0 {
            abort_invalid_free();
        }
        (SIZE_CLASSES[index / ARENA_SHARDS], DEFAULT_ALIGNMENT)
    } else {
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
    if size <= old_size {
        return pointer;
    }
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

fn allocation_usable_size_for(size: usize, alignment: usize) -> Option<usize> {
    if !valid_alignment(alignment) {
        return None;
    }
    let requested = cmp::max(size, 1);
    if alignment <= DEFAULT_ALIGNMENT {
        if let Some(index) = class_index(requested) {
            return Some(SIZE_CLASSES[index]);
        }
    }
    guarded_mapping_geometry(size, alignment, page_size()).map(|_| requested)
}

fn reallocate_with_options(
    pointer: *mut c_void,
    size: usize,
    alignment: usize,
    zero: bool,
) -> *mut c_void {
    if pointer.is_null() {
        return allocate(size, alignment, zero);
    }
    if size == 0 {
        deallocate(pointer);
        return ptr::null_mut();
    }
    if !valid_alignment(alignment) {
        set_errno(libc::EINVAL);
        return ptr::null_mut();
    }
    let Some(old_size) = allocation_size(pointer) else {
        abort_invalid_free();
    };
    if size <= old_size && (pointer as usize).is_multiple_of(alignment) {
        return pointer;
    }
    let replacement = allocate(size, alignment, zero);
    if replacement.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: both allocations are live and non-overlapping. The replacement
    // is zero-filled before the preserved prefix is copied when requested.
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
/// The C `malloc` entry point.
///
/// # Safety
///
/// Callers follow the C allocation contract for the returned block.
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    allocate(size, DEFAULT_ALIGNMENT, false)
}

#[cfg(not(test))]
#[no_mangle]
/// The C `free` entry point.
///
/// # Safety
///
/// `pointer` is null or a live block from this allocator, as C requires.
pub unsafe extern "C" fn free(pointer: *mut c_void) {
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
/// The C `realloc` entry point.
///
/// # Safety
///
/// `pointer` is null or a live block from this allocator, as C requires.
pub unsafe extern "C" fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
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

fn mallocx_options(flags: c_int) -> Option<(usize, bool)> {
    const SUPPORTED_FLAGS: c_int = 0x7f;
    if flags < 0 || flags & !SUPPORTED_FLAGS != 0 {
        return None;
    }
    let alignment = mallocx_alignment(flags);
    valid_alignment(alignment).then_some((alignment, flags & 0x40 != 0))
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn mallocx(size: usize, flags: c_int) -> *mut c_void {
    let Some((alignment, zero)) = mallocx_options(flags) else {
        set_errno(libc::EINVAL);
        return ptr::null_mut();
    };
    allocate(size, alignment, zero)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn rallocx(pointer: *mut c_void, size: usize, flags: c_int) -> *mut c_void {
    let Some((alignment, zero)) = mallocx_options(flags) else {
        set_errno(libc::EINVAL);
        return ptr::null_mut();
    };
    reallocate_with_options(pointer, size, alignment, zero)
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
    mallocx_options(flags)
        .and_then(|(alignment, _)| allocation_usable_size_for(size, alignment))
        .unwrap_or(0)
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
    static MESSAGE: &[u8] = b"ginux-allocator: guarded arena allocator\n\0";
    if let Some(write) = callback {
        // SAFETY: MESSAGE is static and NUL terminated as required by the API.
        unsafe { write(opaque, MESSAGE.as_ptr().cast::<c_char>()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_ALLOCATOR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn allocation_is_aligned_guarded_and_tracked() {
        let _guard = TEST_ALLOCATOR_LOCK.lock().expect("allocator test lock");
        let pointer = allocate(123, 64, false);
        assert!(!pointer.is_null());
        assert_eq!(pointer as usize % 64, 0);
        assert_eq!(allocation_size(pointer), Some(123));
        assert!(LIVE_ALLOCATIONS.load(Ordering::Relaxed) >= 1);
        deallocate(pointer);
        assert_eq!(LIVE_ALLOCATIONS.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn calloc_and_reallocate_preserve_data() {
        let _guard = TEST_ALLOCATOR_LOCK.lock().expect("allocator test lock");
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

    #[test]
    fn guarded_geometry_is_checked_and_contains_requested_bytes() {
        for alignment in [16, 64, 4096, MAX_ALIGNMENT] {
            for size in [0, 1, alignment, 65_537, usize::MAX] {
                if let Some((requested, data, mapping)) =
                    guarded_mapping_geometry(size, alignment, 4096)
                {
                    assert!(requested >= 1);
                    assert!(data >= requested);
                    assert!(mapping >= data + 8192);
                    assert_eq!(data % 4096, 0);
                } else {
                    assert_eq!(size, usize::MAX);
                }
            }
        }
        assert!(guarded_mapping_geometry(1, 3, 4096).is_none());
        assert!(guarded_mapping_geometry(1, 16, 0).is_none());
    }

    #[test]
    fn bounded_accounting_model_never_wraps_or_exceeds_limit() {
        for limit in 0usize..=64 {
            for current in 0usize..=80 {
                for additional in 0usize..=80 {
                    let result = bounded_add(current, additional, limit);
                    assert_eq!(
                        result,
                        current
                            .checked_add(additional)
                            .filter(|next| *next <= limit)
                    );
                }
            }
        }
        assert_eq!(bounded_add(usize::MAX, 1, usize::MAX), None);
    }

    #[test]
    fn size_class_model_is_monotonic_and_complete() {
        for pair in SIZE_CLASSES.windows(2) {
            assert!(pair[0] < pair[1]);
            assert_eq!(pair[1], pair[0] * 2);
        }
        for size in 1usize..=*SIZE_CLASSES.last().unwrap() {
            let index = class_index(size).expect("covered allocation size");
            assert!(SIZE_CLASSES[index] >= size);
            if index > 0 {
                assert!(SIZE_CLASSES[index - 1] < size);
            }
        }
    }

    #[test]
    fn shard_fallback_visits_every_arena_once() {
        for preferred in 0..ARENA_SHARDS {
            let mut seen = 0usize;
            for probe in 0..ARENA_SHARDS {
                let shard = shard_probe(preferred, probe);
                assert!(shard < ARENA_SHARDS);
                let bit = 1usize << shard;
                assert_eq!(seen & bit, 0);
                seen |= bit;
            }
            assert_eq!(seen, (1usize << ARENA_SHARDS) - 1);
        }
    }

    #[test]
    fn full_arena_fallback_never_invents_capacity_and_freed_slots_recover() {
        let _guard = TEST_ALLOCATOR_LOCK.lock().expect("allocator test lock");
        let class_index = SIZE_CLASSES.len() - 1;
        let class = SIZE_CLASSES[class_index];
        let slots_per_arena = ARENA_BYTES / class;
        let live_before = LIVE_BYTES.load(Ordering::Acquire);
        let mut pointers = Vec::new();
        for _ in 0..ARENA_SHARDS * slots_per_arena {
            let pointer = allocate(class, DEFAULT_ALIGNMENT, false);
            assert!(!pointer.is_null());
            pointers.push(pointer);
        }
        let reserved_before_failure: usize = (0..ARENA_SHARDS)
            .map(|shard| {
                ARENAS[class_index * ARENA_SHARDS + shard]
                    .reserved_bytes
                    .load(Ordering::Acquire)
            })
            .sum();
        assert_eq!(reserved_before_failure, ARENA_SHARDS * ARENA_BYTES);
        assert!(allocate(class, DEFAULT_ALIGNMENT, false).is_null());
        let reserved_after_failure: usize = (0..ARENA_SHARDS)
            .map(|shard| {
                ARENAS[class_index * ARENA_SHARDS + shard]
                    .reserved_bytes
                    .load(Ordering::Acquire)
            })
            .sum();
        assert_eq!(reserved_after_failure, reserved_before_failure);

        for pointer in pointers {
            deallocate(pointer);
        }
        for shard in 0..ARENA_SHARDS {
            assert_eq!(
                ARENAS[class_index * ARENA_SHARDS + shard]
                    .live_bytes
                    .load(Ordering::Acquire),
                0
            );
        }
        assert_eq!(LIVE_BYTES.load(Ordering::Acquire), live_before);
        let reused = allocate(class, DEFAULT_ALIGNMENT, false);
        assert!(!reused.is_null());
        let reserved_after_reuse: usize = (0..ARENA_SHARDS)
            .map(|shard| {
                ARENAS[class_index * ARENA_SHARDS + shard]
                    .reserved_bytes
                    .load(Ordering::Acquire)
            })
            .sum();
        assert_eq!(reserved_after_reuse, reserved_before_failure);
        deallocate(reused);
        assert_eq!(LIVE_BYTES.load(Ordering::Acquire), live_before);
    }

    #[test]
    fn extended_reallocation_honors_alignment_zeroing_and_size_prediction() {
        let _guard = TEST_ALLOCATOR_LOCK.lock().expect("allocator test lock");
        let initial = allocate(33, 64, false);
        assert!(!initial.is_null());
        // SAFETY: initial is live for 33 bytes.
        unsafe { ptr::write_bytes(initial.cast::<u8>(), 0x5a, 33) };
        let (alignment, zero) = mallocx_options(12 | 0x40).expect("supported flags");
        let grown = reallocate_with_options(initial, 4097, alignment, zero);
        assert!(!grown.is_null());
        assert_eq!(grown as usize % 4096, 0);
        assert_eq!(
            allocation_size(grown),
            allocation_usable_size_for(4097, 4096)
        );
        // SAFETY: grown is live for 4097 bytes.
        let bytes = unsafe { core::slice::from_raw_parts(grown.cast::<u8>(), 4097) };
        assert!(bytes[..33].iter().all(|byte| *byte == 0x5a));
        assert!(bytes[33..].iter().all(|byte| *byte == 0));
        deallocate(grown);
        assert!(mallocx_options(0x80).is_none());
    }

    fn nallocx_for_test(size: usize, lg_alignment: usize) -> usize {
        let alignment = 1usize << lg_alignment;
        allocation_usable_size_for(size, alignment).unwrap_or(0)
    }
}

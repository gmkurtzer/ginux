use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use ginux_oci::{Entry, EntryKind, Rootfs};

const ALPINE_ROOTFS_SHA256: [u8; 32] = [
    0x74, 0xd9, 0x7c, 0x42, 0x8c, 0x51, 0xa8, 0x28, 0xf9, 0x05, 0x1a, 0x7a, 0x40, 0xa5, 0x3f, 0xf1,
    0xfc, 0x99, 0xe5, 0x4f, 0xc3, 0x03, 0x23, 0xce, 0x36, 0x76, 0x07, 0x01, 0xb0, 0xb7, 0xf7, 0x11,
];

static INSTALLED: AtomicBool = AtomicBool::new(false);
static ADDRESS: AtomicUsize = AtomicUsize::new(0);
static SIZE: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn install_alpine_rootfs(bytes: &'static [u8]) -> bool {
    if INSTALLED.load(Ordering::Acquire) || !ginux_oci::verify_sha256(bytes, &ALPINE_ROOTFS_SHA256)
    {
        return false;
    }
    let Some(rootfs) = Rootfs::parse(bytes) else {
        return false;
    };
    let valid = rootfs
        .find(b"bin/sh")
        .is_some_and(|entry| entry.kind == EntryKind::Symlink && entry.link == b"/bin/busybox")
        && rootfs.find(b"bin/busybox").is_some_and(|entry| {
            entry.kind == EntryKind::File
                && entry.mode & 0o111 != 0
                && entry.data.starts_with(b"\x7fELF")
        })
        && rootfs
            .find(b"lib/ld-musl-x86_64.so.1")
            .is_some_and(|entry| {
                entry.kind == EntryKind::File
                    && entry.mode & 0o111 != 0
                    && entry.data.starts_with(b"\x7fELF")
            });
    if !valid {
        return false;
    }
    ADDRESS.store(bytes.as_ptr() as usize, Ordering::Relaxed);
    SIZE.store(bytes.len(), Ordering::Relaxed);
    INSTALLED.store(true, Ordering::Release);
    file(b"bin/busybox").is_some()
}

pub(crate) fn file(path: &[u8]) -> Option<Entry<'static>> {
    if !INSTALLED.load(Ordering::Acquire) {
        return None;
    }
    let address = ADDRESS.load(Ordering::Relaxed);
    let size = SIZE.load(Ordering::Relaxed);
    if address == 0 || size == 0 {
        return None;
    }
    // SAFETY: installation records an immutable slice embedded in the loaded
    // EFI image. The image remains mapped and read-only for the kernel lifetime.
    let bytes = unsafe { core::slice::from_raw_parts(address as *const u8, size) };
    Rootfs::parse(bytes)?.find(path)
}

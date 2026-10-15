#![no_std]

#[cfg(test)]
extern crate std;

use sha2::{Digest as ShaDigest, Sha256};

pub const SHA256_SIZE: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Digest(pub [u8; SHA256_SIZE]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StagingHandle {
    slot: u16,
    generation: u32,
}

impl StagingHandle {
    pub const fn into_raw(self) -> u64 {
        (self.generation as u64) << 16 | self.slot as u64
    }

    pub const fn from_raw(raw: u64) -> Option<Self> {
        let slot = raw as u16;
        let generation = (raw >> 16) as u32;
        if raw >> 48 != 0 || generation == 0 {
            None
        } else {
            Some(Self { slot, generation })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreError {
    InvalidHandle,
    InvalidLength,
    NoObjectSlot,
    QuotaExceeded,
    Backend,
    DigestMismatch,
    AlreadyPresent,
}

pub trait ObjectBackend {
    type Object: Copy + Eq;

    fn create(&mut self, expected_length: usize) -> Option<Self::Object>;
    fn append(&mut self, object: Self::Object, offset: usize, bytes: &[u8]) -> bool;
    fn read(&self, object: Self::Object, offset: usize, output: &mut [u8]) -> bool;
    fn destroy(&mut self, object: Self::Object) -> bool;
}

#[derive(Clone)]
struct Entry<O: Copy> {
    generation: u32,
    object: O,
    expected: Digest,
    expected_length: usize,
    written: usize,
    hasher: Sha256,
    published: bool,
}

pub struct ImageStore<B: ObjectBackend, const OBJECTS: usize> {
    backend: B,
    entries: [Option<Entry<B::Object>>; OBJECTS],
    next_generation: u32,
    quota: usize,
    reserved: usize,
    published: usize,
}

impl<B: ObjectBackend, const OBJECTS: usize> ImageStore<B, OBJECTS> {
    pub fn new(backend: B, quota: usize) -> Self {
        Self {
            backend,
            entries: core::array::from_fn(|_| None),
            next_generation: 0,
            quota,
            reserved: 0,
            published: 0,
        }
    }

    pub fn stage(
        &mut self,
        expected: Digest,
        expected_length: usize,
    ) -> Result<StagingHandle, StoreError> {
        if expected_length == 0 {
            return Err(StoreError::InvalidLength);
        }
        if self.find(expected).is_some() {
            return Err(StoreError::AlreadyPresent);
        }
        let slot = self
            .entries
            .iter()
            .position(Option::is_none)
            .ok_or(StoreError::NoObjectSlot)?;
        if self
            .reserved
            .checked_add(expected_length)
            .filter(|total| *total <= self.quota)
            .is_none()
        {
            return Err(StoreError::QuotaExceeded);
        }
        let generation = self
            .next_generation
            .checked_add(1)
            .ok_or(StoreError::NoObjectSlot)?;
        let object = self
            .backend
            .create(expected_length)
            .ok_or(StoreError::Backend)?;
        self.next_generation = generation;
        self.reserved += expected_length;
        self.entries[slot] = Some(Entry {
            generation,
            object,
            expected,
            expected_length,
            written: 0,
            hasher: Sha256::new(),
            published: false,
        });
        Ok(StagingHandle {
            slot: slot as u16,
            generation,
        })
    }

    pub fn append(&mut self, handle: StagingHandle, bytes: &[u8]) -> Result<usize, StoreError> {
        let slot = usize::from(handle.slot);
        let entry = self
            .entries
            .get(slot)
            .and_then(Option::as_ref)
            .filter(|entry| entry.generation == handle.generation)
            .ok_or(StoreError::InvalidHandle)?;
        if entry.published
            || entry
                .written
                .checked_add(bytes.len())
                .filter(|end| *end <= entry.expected_length)
                .is_none()
        {
            return Err(StoreError::InvalidLength);
        }
        let object = entry.object;
        let offset = entry.written;
        if !self.backend.append(object, offset, bytes) {
            return Err(StoreError::Backend);
        }
        let entry = self.entries[slot]
            .as_mut()
            .ok_or(StoreError::InvalidHandle)?;
        entry.hasher.update(bytes);
        entry.written += bytes.len();
        Ok(entry.written)
    }

    pub fn publish(&mut self, handle: StagingHandle) -> Result<Digest, StoreError> {
        let entry = self.entry_mut(handle)?;
        if entry.published || entry.written != entry.expected_length {
            return Err(StoreError::InvalidLength);
        }
        let actual = Digest(entry.hasher.clone().finalize().into());
        if actual != entry.expected {
            self.abort(handle)?;
            return Err(StoreError::DigestMismatch);
        }
        let entry = self.entry_mut(handle)?;
        entry.published = true;
        self.published += entry.expected_length;
        Ok(actual)
    }

    pub fn abort(&mut self, handle: StagingHandle) -> Result<(), StoreError> {
        let slot = usize::from(handle.slot);
        let entry = self
            .entries
            .get_mut(slot)
            .and_then(Option::take)
            .filter(|entry| entry.generation == handle.generation && !entry.published)
            .ok_or(StoreError::InvalidHandle)?;
        if !self.backend.destroy(entry.object) {
            self.entries[slot] = Some(entry);
            return Err(StoreError::Backend);
        }
        self.reserved -= entry.expected_length;
        Ok(())
    }

    pub fn contains(&self, digest: Digest) -> bool {
        self.find(digest).is_some()
    }

    pub fn read(&self, digest: Digest, offset: usize, output: &mut [u8]) -> Option<usize> {
        let entry = self
            .entries
            .iter()
            .flatten()
            .find(|entry| entry.published && entry.expected == digest)?;
        let length = output
            .len()
            .min(entry.expected_length.saturating_sub(offset));
        self.backend
            .read(entry.object, offset, &mut output[..length])
            .then_some(length)
    }

    pub fn published_object(&self, digest: Digest) -> Option<(B::Object, usize)> {
        self.entries
            .iter()
            .flatten()
            .find(|entry| entry.published && entry.expected == digest)
            .map(|entry| (entry.object, entry.expected_length))
    }

    pub fn remove(&mut self, digest: Digest) -> Result<(), StoreError> {
        let slot = self
            .entries
            .iter()
            .position(|entry| {
                entry
                    .as_ref()
                    .is_some_and(|entry| entry.published && entry.expected == digest)
            })
            .ok_or(StoreError::InvalidHandle)?;
        let entry = self.entries[slot].take().ok_or(StoreError::InvalidHandle)?;
        if !self.backend.destroy(entry.object) {
            self.entries[slot] = Some(entry);
            return Err(StoreError::Backend);
        }
        self.reserved -= entry.expected_length;
        self.published -= entry.expected_length;
        Ok(())
    }

    pub const fn quota(&self) -> usize {
        self.quota
    }

    pub const fn reserved_bytes(&self) -> usize {
        self.reserved
    }

    pub const fn published_bytes(&self) -> usize {
        self.published
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    fn entry_mut(&mut self, handle: StagingHandle) -> Result<&mut Entry<B::Object>, StoreError> {
        self.entries
            .get_mut(usize::from(handle.slot))
            .and_then(Option::as_mut)
            .filter(|entry| entry.generation == handle.generation)
            .ok_or(StoreError::InvalidHandle)
    }

    fn find(&self, digest: Digest) -> Option<&Entry<B::Object>> {
        self.entries
            .iter()
            .flatten()
            .find(|entry| entry.published && entry.expected == digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    use std::vec::Vec;

    #[derive(Default)]
    struct Backend {
        objects: Vec<Option<Vec<u8>>>,
    }

    impl ObjectBackend for Backend {
        type Object = usize;

        fn create(&mut self, expected_length: usize) -> Option<Self::Object> {
            let slot = self.objects.len();
            self.objects.push(Some(Vec::with_capacity(expected_length)));
            Some(slot)
        }

        fn append(&mut self, object: Self::Object, offset: usize, bytes: &[u8]) -> bool {
            let Some(buffer) = self.objects.get_mut(object).and_then(Option::as_mut) else {
                return false;
            };
            if buffer.len() != offset {
                return false;
            }
            buffer.extend_from_slice(bytes);
            true
        }

        fn read(&self, object: Self::Object, offset: usize, output: &mut [u8]) -> bool {
            self.objects
                .get(object)
                .and_then(Option::as_ref)
                .and_then(|bytes| bytes.get(offset..offset + output.len()))
                .is_some_and(|bytes| {
                    output.copy_from_slice(bytes);
                    true
                })
        }

        fn destroy(&mut self, object: Self::Object) -> bool {
            self.objects
                .get_mut(object)
                .is_some_and(|object| object.take().is_some())
        }
    }

    fn digest(bytes: &[u8]) -> Digest {
        Digest(Sha256::digest(bytes).into())
    }

    #[test]
    fn publishes_only_complete_verified_objects() {
        let payload = b"manifest bytes";
        let expected = digest(payload);
        let mut store = ImageStore::<Backend, 2>::new(Backend::default(), 64);
        let handle = store.stage(expected, payload.len()).unwrap();
        store.append(handle, &payload[..5]).unwrap();
        assert!(!store.contains(expected));
        store.append(handle, &payload[5..]).unwrap();
        assert_eq!(store.publish(handle), Ok(expected));
        let mut output = [0_u8; 32];
        let length = store.read(expected, 0, &mut output).unwrap();
        assert_eq!(&output[..length], payload);
        assert_eq!(store.published_bytes(), payload.len());
    }

    #[test]
    fn digest_failure_aborts_and_releases_quota() {
        let mut store = ImageStore::<Backend, 1>::new(Backend::default(), 4);
        let handle = store.stage(digest(b"good"), 4).unwrap();
        store.append(handle, b"evil").unwrap();
        assert_eq!(store.publish(handle), Err(StoreError::DigestMismatch));
        assert_eq!(store.reserved_bytes(), 0);
        assert_eq!(store.backend().objects, vec![None]);
    }

    #[test]
    fn quota_and_stale_handles_are_enforced() {
        let mut store = ImageStore::<Backend, 2>::new(Backend::default(), 5);
        let first = store.stage(digest(b"1234"), 4).unwrap();
        assert_eq!(
            store.stage(digest(b"12"), 2),
            Err(StoreError::QuotaExceeded)
        );
        store.abort(first).unwrap();
        let second = store.stage(digest(b"12"), 2).unwrap();
        assert_eq!(store.append(first, b"x"), Err(StoreError::InvalidHandle));
        store.append(second, b"12").unwrap();
        assert!(store.publish(second).is_ok());
    }

    #[test]
    fn staging_handles_round_trip_without_ambient_authority() {
        let mut store = ImageStore::<Backend, 1>::new(Backend::default(), 4);
        let handle = store.stage(digest(b"1234"), 4).unwrap();
        assert_eq!(StagingHandle::from_raw(handle.into_raw()), Some(handle));
        assert!(StagingHandle::from_raw(0).is_none());
        assert!(StagingHandle::from_raw(u64::MAX).is_none());
    }

    #[test]
    fn removing_a_published_object_releases_its_slot_and_quota() {
        let payload = b"1234";
        let expected = digest(payload);
        let mut store = ImageStore::<Backend, 1>::new(Backend::default(), payload.len());
        let first = store.stage(expected, payload.len()).unwrap();
        store.append(first, payload).unwrap();
        store.publish(first).unwrap();
        assert_eq!(store.reserved_bytes(), payload.len());
        assert_eq!(store.published_bytes(), payload.len());
        assert_eq!(store.remove(expected), Ok(()));
        assert_eq!(store.reserved_bytes(), 0);
        assert_eq!(store.published_bytes(), 0);
        assert!(!store.contains(expected));
        assert!(store.stage(expected, payload.len()).is_ok());
    }
}

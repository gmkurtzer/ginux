#![no_std]

#[cfg(test)]
extern crate std;

pub const SLOTS_PER_METADATA_PAGE: usize = 254;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct RuntimeMappingSlot {
    pub frame: u64,
    pub generation: u32,
    pub protection: u8,
    pub file: u8,
    pub flags: u16,
}

impl RuntimeMappingSlot {
    pub const EMPTY: Self = Self {
        frame: 0,
        generation: 0,
        protection: 0,
        file: 0,
        flags: 0,
    };
}

pub trait MetadataBackend {
    type Page: Copy + Eq;

    fn allocate_page(&mut self) -> Option<Self::Page>;
    fn release_page(&mut self, page: Self::Page) -> bool;
    fn next_page(&self, page: Self::Page) -> Option<Self::Page>;
    fn set_next_page(&mut self, page: Self::Page, next: Self::Page) -> bool;
    fn read_slot(&self, page: Self::Page, slot: usize) -> Option<RuntimeMappingSlot>;
    fn write_slot(&mut self, page: Self::Page, slot: usize, value: RuntimeMappingSlot) -> bool;
}

pub struct RuntimeMappingMetadata<B: MetadataBackend> {
    backend: B,
    root: Option<B::Page>,
    page_count: usize,
}

impl<B: MetadataBackend> RuntimeMappingMetadata<B> {
    pub const fn new(backend: B) -> Self {
        Self {
            backend,
            root: None,
            page_count: 0,
        }
    }

    pub const fn allocated_pages(&self) -> usize {
        self.page_count
    }

    pub fn read(&self, slot: usize) -> Option<RuntimeMappingSlot> {
        let page = self.page(slot / SLOTS_PER_METADATA_PAGE)?;
        self.backend.read_slot(page, slot % SLOTS_PER_METADATA_PAGE)
    }

    pub fn write(&mut self, slot: usize, value: RuntimeMappingSlot) -> bool {
        let Some(page) = self.ensure_page(slot / SLOTS_PER_METADATA_PAGE) else {
            return false;
        };
        self.backend
            .write_slot(page, slot % SLOTS_PER_METADATA_PAGE, value)
    }

    pub fn release(&mut self) -> bool {
        let mut current = self.root;
        let mut released = 0;
        while let Some(page) = current {
            if released >= self.page_count {
                return false;
            }
            let next = self.backend.next_page(page);
            if !self.backend.release_page(page) {
                return false;
            }
            current = next;
            released += 1;
        }
        if released != self.page_count {
            return false;
        }
        self.root = None;
        self.page_count = 0;
        true
    }

    pub fn into_backend(self) -> B {
        self.backend
    }

    fn page(&self, target: usize) -> Option<B::Page> {
        if target >= self.page_count {
            return None;
        }
        let mut page = self.root?;
        for _ in 0..target {
            page = self.backend.next_page(page)?;
        }
        Some(page)
    }

    fn ensure_page(&mut self, target: usize) -> Option<B::Page> {
        if self.root.is_none() {
            self.root = Some(self.backend.allocate_page()?);
            self.page_count = 1;
        }
        let mut page = self.root?;
        for index in 0..=target {
            if index == target {
                return Some(page);
            }
            page = if let Some(next) = self.backend.next_page(page) {
                next
            } else {
                let next_count = self.page_count.checked_add(1)?;
                let allocated = self.backend.allocate_page()?;
                if !self.backend.set_next_page(page, allocated) {
                    let _ = self.backend.release_page(allocated);
                    return None;
                }
                self.page_count = next_count;
                allocated
            };
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[derive(Clone)]
    struct Page {
        next: Option<usize>,
        slots: [RuntimeMappingSlot; SLOTS_PER_METADATA_PAGE],
        live: bool,
    }

    struct Backend {
        pages: Vec<Page>,
        allocation_limit: Option<usize>,
    }

    impl Backend {
        fn unlimited() -> Self {
            Self {
                pages: Vec::new(),
                allocation_limit: None,
            }
        }
    }

    impl MetadataBackend for Backend {
        type Page = usize;

        fn allocate_page(&mut self) -> Option<Self::Page> {
            if self
                .allocation_limit
                .is_some_and(|limit| self.pages.len() >= limit)
            {
                return None;
            }
            let page = self.pages.len();
            self.pages.push(Page {
                next: None,
                slots: [RuntimeMappingSlot::EMPTY; SLOTS_PER_METADATA_PAGE],
                live: true,
            });
            Some(page)
        }

        fn release_page(&mut self, page: Self::Page) -> bool {
            let Some(page) = self.pages.get_mut(page).filter(|page| page.live) else {
                return false;
            };
            page.live = false;
            true
        }

        fn next_page(&self, page: Self::Page) -> Option<Self::Page> {
            self.pages.get(page).filter(|page| page.live)?.next
        }

        fn set_next_page(&mut self, page: Self::Page, next: Self::Page) -> bool {
            let next_live = self.pages.get(next).is_some_and(|page| page.live);
            let Some(page) = self.pages.get_mut(page).filter(|page| page.live) else {
                return false;
            };
            if page.next.is_some() || !next_live {
                return false;
            }
            page.next = Some(next);
            true
        }

        fn read_slot(&self, page: Self::Page, slot: usize) -> Option<RuntimeMappingSlot> {
            self.pages
                .get(page)
                .filter(|page| page.live)?
                .slots
                .get(slot)
                .copied()
        }

        fn write_slot(&mut self, page: Self::Page, slot: usize, value: RuntimeMappingSlot) -> bool {
            let Some(slot) = self
                .pages
                .get_mut(page)
                .filter(|page| page.live)
                .and_then(|page| page.slots.get_mut(slot))
            else {
                return false;
            };
            *slot = value;
            true
        }
    }

    #[test]
    fn grows_metadata_without_a_fixed_mapping_limit() {
        let mut metadata = RuntimeMappingMetadata::new(Backend::unlimited());
        let first = RuntimeMappingSlot {
            frame: 0x20_0000,
            generation: 7,
            protection: 3,
            file: 0,
            flags: 1,
        };
        let distant_slot = SLOTS_PER_METADATA_PAGE * 16 + 7;
        let distant = RuntimeMappingSlot {
            frame: 0x30_0000,
            generation: 11,
            protection: 1,
            file: 4,
            flags: 0,
        };
        assert!(metadata.write(0, first));
        assert!(metadata.write(distant_slot, distant));
        assert_eq!(metadata.allocated_pages(), 17);
        assert_eq!(metadata.read(0), Some(first));
        assert_eq!(
            metadata.read(SLOTS_PER_METADATA_PAGE * 8),
            Some(RuntimeMappingSlot::EMPTY)
        );
        assert_eq!(metadata.read(distant_slot), Some(distant));
        assert!(metadata.release());
        assert_eq!(metadata.allocated_pages(), 0);
        assert!(metadata.into_backend().pages.iter().all(|page| !page.live));
    }

    #[test]
    fn allocation_failure_does_not_publish_a_partial_link() {
        let backend = Backend {
            pages: Vec::new(),
            allocation_limit: Some(1),
        };
        let mut metadata = RuntimeMappingMetadata::new(backend);
        assert!(!metadata.write(
            SLOTS_PER_METADATA_PAGE,
            RuntimeMappingSlot {
                frame: 0x20_0000,
                generation: 1,
                protection: 1,
                file: 0,
                flags: 0,
            }
        ));
        assert_eq!(metadata.allocated_pages(), 1);
        assert_eq!(metadata.read(0), Some(RuntimeMappingSlot::EMPTY));
        assert!(metadata.release());
    }
}

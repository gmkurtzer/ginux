#![no_std]

pub const NATIVE_SERVICE_BOOT_MAGIC: u64 = 0x3042_5358_554e_4947;
pub const NATIVE_SERVICE_BOOT_VERSION: u16 = 0;
pub const NATIVE_SERVICE_BOOT_SIZE: u16 = 64;
pub const NATIVE_SERVICE_FEATURE_VIRTIO_BLOCK: u32 = 1 << 0;
pub const NATIVE_SERVICE_EXIT_OPERATION: u64 = 2;
pub const NATIVE_SERVICE_EXIT_COMPLETED: u8 = 0;
pub const NATIVE_SERVICE_EXIT_REJECTED: u8 = 1;
pub const NATIVE_SERVICE_EXIT_RESTART_REQUIRED: u8 = 2;
pub const NATIVE_BLOCK_WAIT_OPERATION: u64 = 19;
pub const NATIVE_BLOCK_WAIT_PREPARED: u64 = 1;
pub const NATIVE_BLOCK_WAIT_NOTIFIED: u64 = 2;
pub const NATIVE_BLOCK_WAIT_FAILED: u64 = u64::MAX - 2;
pub const NATIVE_BLOCK_WAIT_CANCELLED: u64 = u64::MAX - 5;
pub const NATIVE_CONTROL_DISCOVERY_OPERATION: u64 = 0;
pub const NATIVE_CONTROL_ABI_VERSION: u16 = 4;
pub const NATIVE_CONTROL_FEATURE_OPAQUE_CAPABILITIES: u16 = 1 << 0;
pub const NATIVE_CONTROL_FEATURE_FAULT_CONTAINMENT: u16 = 1 << 4;
pub const NATIVE_CONTROL_REQUIRED_FEATURES: u16 =
    NATIVE_CONTROL_FEATURE_OPAQUE_CAPABILITIES | NATIVE_CONTROL_FEATURE_FAULT_CONTAINMENT;

pub const fn native_control_abi_is_compatible(discovery: u64) -> bool {
    let version = ((discovery >> 16) & u16::MAX as u64) as u16;
    let features = (discovery & u16::MAX as u64) as u16;
    discovery >> 32 == 0
        && version == NATIVE_CONTROL_ABI_VERSION
        && features & NATIVE_CONTROL_REQUIRED_FEATURES == NATIVE_CONTROL_REQUIRED_FEATURES
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeServiceBootV0 {
    pub magic: u64,
    pub version: u16,
    pub size: u16,
    pub features: u32,
    pub common_config_address: u64,
    pub queue_notification_address: u64,
    pub dma_virtual_address: u64,
    pub dma_device_address: u64,
    pub device_capability: u64,
    pub reserved: u64,
}

impl NativeServiceBootV0 {
    pub const fn virtio_block(
        common_config_address: u64,
        queue_notification_address: u64,
        dma_virtual_address: u64,
        dma_device_address: u64,
        device_capability: u64,
    ) -> Self {
        Self {
            magic: NATIVE_SERVICE_BOOT_MAGIC,
            version: NATIVE_SERVICE_BOOT_VERSION,
            size: NATIVE_SERVICE_BOOT_SIZE,
            features: NATIVE_SERVICE_FEATURE_VIRTIO_BLOCK,
            common_config_address,
            queue_notification_address,
            dma_virtual_address,
            dma_device_address,
            device_capability,
            reserved: 0,
        }
    }

    pub const fn is_valid_virtio_block(&self) -> bool {
        self.magic == NATIVE_SERVICE_BOOT_MAGIC
            && self.version == NATIVE_SERVICE_BOOT_VERSION
            && self.size == NATIVE_SERVICE_BOOT_SIZE
            && self.features == NATIVE_SERVICE_FEATURE_VIRTIO_BLOCK
            && self.common_config_address != 0
            && self.queue_notification_address != 0
            && self.dma_virtual_address != 0
            && self.dma_device_address != 0
            && self.device_capability != 0
            && self.reserved == 0
    }
}

const _: () = assert!(core::mem::size_of::<NativeServiceBootV0>() == 64);
const _: () = assert!(core::mem::align_of::<NativeServiceBootV0>() == 8);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_and_header_are_frozen() {
        assert_eq!(NATIVE_SERVICE_BOOT_MAGIC.to_le_bytes(), *b"GINUXSB0");
        assert_eq!(core::mem::size_of::<NativeServiceBootV0>(), 64);
        assert_eq!(core::mem::align_of::<NativeServiceBootV0>(), 8);
        assert_eq!(core::mem::offset_of!(NativeServiceBootV0, magic), 0);
        assert_eq!(core::mem::offset_of!(NativeServiceBootV0, version), 8);
        assert_eq!(core::mem::offset_of!(NativeServiceBootV0, size), 10);
        assert_eq!(core::mem::offset_of!(NativeServiceBootV0, features), 12);
        assert_eq!(
            core::mem::offset_of!(NativeServiceBootV0, common_config_address),
            16
        );
        assert_eq!(
            core::mem::offset_of!(NativeServiceBootV0, device_capability),
            48
        );
        assert_eq!(core::mem::offset_of!(NativeServiceBootV0, reserved), 56);
    }

    #[test]
    fn c_binding_tracks_the_canonical_contract() {
        let header = include_str!("../../../specs/abi/native-service-bootstrap-v0.h");
        assert!(header.contains("UINT64_C(0x30425358554e4947)"));
        assert!(header.contains("GINUX_NATIVE_SERVICE_BOOT_VERSION UINT16_C(0)"));
        assert!(header.contains("GINUX_NATIVE_SERVICE_BOOT_SIZE UINT16_C(64)"));
        assert!(header.contains("GINUX_NATIVE_SERVICE_FEATURE_VIRTIO_BLOCK UINT32_C(1)"));
        assert!(header.contains("sizeof(struct ginux_native_service_boot_v0) == 64"));
    }

    #[test]
    fn lifecycle_binding_tracks_the_canonical_contract() {
        let header = include_str!("../../../specs/abi/native-service-lifecycle-v0.h");
        assert!(header.contains("GINUX_NATIVE_SERVICE_EXIT_OPERATION UINT64_C(2)"));
        assert!(header.contains("GINUX_NATIVE_SERVICE_EXIT_COMPLETED UINT8_C(0)"));
        assert!(header.contains("GINUX_NATIVE_SERVICE_EXIT_REJECTED UINT8_C(1)"));
        assert!(header.contains("GINUX_NATIVE_SERVICE_EXIT_RESTART_REQUIRED UINT8_C(2)"));
        assert!(header.contains("GINUX_NATIVE_BLOCK_WAIT_OPERATION UINT64_C(19)"));
        assert!(header.contains("GINUX_NATIVE_BLOCK_WAIT_PREPARED UINT64_C(1)"));
        assert!(header.contains("GINUX_NATIVE_BLOCK_WAIT_NOTIFIED UINT64_C(2)"));
        assert!(header.contains("GINUX_NATIVE_BLOCK_WAIT_CANCELLED (UINT64_MAX - UINT64_C(5))"));
        assert!(header.contains("GINUX_NATIVE_CONTROL_DISCOVERY_OPERATION UINT64_C(0)"));
        assert!(header.contains("GINUX_NATIVE_CONTROL_ABI_VERSION UINT16_C(4)"));
    }

    #[test]
    fn rejects_header_and_reserved_field_changes() {
        let valid = NativeServiceBootV0::virtio_block(1, 2, 3, 4, 5);
        assert!(valid.is_valid_virtio_block());
        for invalid in [
            NativeServiceBootV0 { magic: 0, ..valid },
            NativeServiceBootV0 {
                version: 1,
                ..valid
            },
            NativeServiceBootV0 { size: 63, ..valid },
            NativeServiceBootV0 {
                features: 0,
                ..valid
            },
            NativeServiceBootV0 {
                reserved: 1,
                ..valid
            },
        ] {
            assert!(!invalid.is_valid_virtio_block());
        }
    }

    #[test]
    fn control_discovery_requires_v4_and_required_features() {
        let compatible = (u64::from(NATIVE_CONTROL_ABI_VERSION) << 16)
            | u64::from(NATIVE_CONTROL_REQUIRED_FEATURES);
        assert!(native_control_abi_is_compatible(compatible));
        assert!(native_control_abi_is_compatible(
            compatible | u64::from(1_u16 << 15)
        ));
        assert!(!native_control_abi_is_compatible(compatible & !1));
        assert!(!native_control_abi_is_compatible(compatible & !(1 << 4)));
        assert!(!native_control_abi_is_compatible(compatible + (1 << 16)));
        assert!(!native_control_abi_is_compatible(
            compatible | (1_u64 << 32)
        ));
    }
}

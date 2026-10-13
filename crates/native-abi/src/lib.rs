#![no_std]

pub const NATIVE_SERVICE_BOOT_MAGIC: u64 = 0x3042_5358_554e_4947;
pub const NATIVE_SERVICE_BOOT_VERSION: u16 = 0;
pub const NATIVE_SERVICE_BOOT_SIZE: u16 = 64;
pub const NATIVE_SERVICE_FEATURE_VIRTIO_BLOCK: u32 = 1 << 0;
pub const NATIVE_NETWORK_BOOT_MAGIC: u64 = 0x3042_4e58_554e_4947;
pub const NATIVE_NETWORK_BOOT_VERSION: u16 = 0;
pub const NATIVE_NETWORK_BOOT_SIZE: u16 = 80;
pub const NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK: u32 = 1 << 0;
pub const NATIVE_NETWORK_FEATURE_EMPTY_QUEUES: u32 = 1 << 1;
pub const NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE: u32 = 1 << 2;
pub const NATIVE_NETWORK_FEATURE_TX_INTERRUPT: u32 = 1 << 3;
pub const NATIVE_NETWORK_FEATURE_ARP_PROBE: u32 = 1 << 4;
pub const NATIVE_NETWORK_FEATURE_RX_REUSE: u32 = 1 << 5;
pub const NATIVE_NETWORK_FEATURE_PACKET_HANDOFF: u32 = 1 << 6;
pub const NATIVE_NETWORK_FEATURE_PACKET_FAULT: u32 = 1 << 7;
pub const NATIVE_NETWORK_FEATURE_PACKET_BATCH: u32 = 1 << 8;
pub const NATIVE_NETWORK_BOOT_V1_VERSION: u16 = 1;
pub const NATIVE_NETWORK_BOOT_V1_SIZE: u16 = 88;
pub const NATIVE_SERVICE_EXIT_OPERATION: u64 = 2;
pub const NATIVE_SERVICE_EXIT_COMPLETED: u8 = 0;
pub const NATIVE_SERVICE_EXIT_REJECTED: u8 = 1;
pub const NATIVE_SERVICE_EXIT_RESTART_REQUIRED: u8 = 2;
pub const NATIVE_BLOCK_WAIT_OPERATION: u64 = 19;
pub const NATIVE_BLOCK_WAIT_PREPARED: u64 = 1;
pub const NATIVE_BLOCK_WAIT_NOTIFIED: u64 = 2;
pub const NATIVE_BLOCK_WAIT_FAILED: u64 = u64::MAX - 2;
pub const NATIVE_BLOCK_WAIT_CANCELLED: u64 = u64::MAX - 5;
pub const NATIVE_NETWORK_DEVICE_OPERATION: u64 = 16;
pub const NATIVE_NETWORK_DEVICE_PROBE: u64 = 0;
pub const NATIVE_NETWORK_DEVICE_ACTIVATE_EMPTY_QUEUES: u64 = 12;
pub const NATIVE_NETWORK_DEVICE_DESCRIPTOR_WORD: u64 = 0x0000_0001_05dc_0009;
pub const NATIVE_NETWORK_WAIT_OPERATION: u64 = 20;
pub const NATIVE_NETWORK_WAIT_TX_NOTIFIED: u64 = 1;
pub const NATIVE_NETWORK_WAIT_RX_POSTED: u64 = 2;
pub const NATIVE_NETWORK_WAIT_FAILED: u64 = u64::MAX - 2;
pub const NATIVE_NETWORK_WAIT_CANCELLED: u64 = u64::MAX - 5;
pub const NATIVE_PACKET_PUBLISH_OPERATION: u64 = 21;
pub const NATIVE_PACKET_CONSUME_OPERATION: u64 = 22;
pub const NATIVE_PACKET_WAIT_OPERATION: u64 = 23;
pub const NATIVE_PACKET_WAIT_TIMEOUT_OPERATION: u64 = 24;
pub const NATIVE_PACKET_SPACE_WAIT_OPERATION: u64 = 25;
pub const NATIVE_PACKET_REQUEST_VERSION: u16 = 0;
pub const NATIVE_PACKET_REQUEST_SIZE: u32 = 32;
pub const NATIVE_PACKET_MAX_SIZE: u32 = 1514;
pub const NATIVE_PACKET_QUEUE_CAPACITY: u32 = 4;
pub const NATIVE_PACKET_WAIT_TIMED_OUT: u64 = u64::MAX - 5;
pub const NATIVE_PACKET_WAIT_MAX_TIMEOUT_MS: u64 = 1_000;
pub const NATIVE_CONTROL_DISCOVERY_OPERATION: u64 = 0;
pub const NATIVE_CONTROL_ABI_VERSION: u16 = 4;
pub const NATIVE_CONTROL_FEATURE_OPAQUE_CAPABILITIES: u16 = 1 << 0;
pub const NATIVE_CONTROL_FEATURE_FAULT_CONTAINMENT: u16 = 1 << 4;
pub const NATIVE_CONTROL_FEATURE_STRUCTURED_DESCRIPTOR: u16 = 1 << 10;
pub const NATIVE_CONTROL_FEATURE_PACKET_HANDOFF: u16 = 1 << 11;
pub const NATIVE_CONTROL_REQUIRED_FEATURES: u16 = NATIVE_CONTROL_FEATURE_OPAQUE_CAPABILITIES
    | NATIVE_CONTROL_FEATURE_FAULT_CONTAINMENT
    | NATIVE_CONTROL_FEATURE_STRUCTURED_DESCRIPTOR;
pub const NATIVE_CONTROL_DESCRIPTOR_OPERATION: u64 = 10;
pub const NATIVE_CONTROL_DESCRIPTOR_SIZE: u32 = 16;
pub const NATIVE_CONTROL_MAX_BUFFERED_CONSOLE_BYTES: u32 = 64;

pub const fn native_control_abi_is_compatible(discovery: u64) -> bool {
    let version = ((discovery >> 16) & u16::MAX as u64) as u16;
    let features = (discovery & u16::MAX as u64) as u16;
    discovery >> 32 == 0
        && version == NATIVE_CONTROL_ABI_VERSION
        && features & NATIVE_CONTROL_REQUIRED_FEATURES == NATIVE_CONTROL_REQUIRED_FEATURES
}

pub const fn native_control_abi_supports_packet_handoff(discovery: u64) -> bool {
    native_control_abi_is_compatible(discovery)
        && discovery as u16 & NATIVE_CONTROL_FEATURE_PACKET_HANDOFF != 0
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeControlDescriptorV4 {
    pub size: u32,
    pub version: u16,
    pub features: u16,
    pub max_buffered_console_bytes: u32,
    pub reserved: u32,
}

impl NativeControlDescriptorV4 {
    pub const fn zeroed() -> Self {
        Self {
            size: 0,
            version: 0,
            features: 0,
            max_buffered_console_bytes: 0,
            reserved: 0,
        }
    }

    pub const fn is_compatible(&self) -> bool {
        self.size == NATIVE_CONTROL_DESCRIPTOR_SIZE
            && self.version == NATIVE_CONTROL_ABI_VERSION
            && self.features & NATIVE_CONTROL_REQUIRED_FEATURES == NATIVE_CONTROL_REQUIRED_FEATURES
            && self.max_buffered_console_bytes == NATIVE_CONTROL_MAX_BUFFERED_CONSOLE_BYTES
            && self.reserved == 0
    }
}

const _: () = assert!(core::mem::size_of::<NativeControlDescriptorV4>() == 16);
const _: () = assert!(core::mem::align_of::<NativeControlDescriptorV4>() == 4);

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

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeNetworkBootV0 {
    pub magic: u64,
    pub version: u16,
    pub size: u16,
    pub features: u32,
    pub common_config_address: u64,
    pub receive_notification_address: u64,
    pub transmit_notification_address: u64,
    pub dma_virtual_address: u64,
    pub dma_device_address: u64,
    pub device_capability: u64,
    pub mac_address: u64,
    pub reserved: u64,
}

impl NativeNetworkBootV0 {
    pub const fn virtio_network(
        common_config_address: u64,
        receive_notification_address: u64,
        transmit_notification_address: u64,
        dma_virtual_address: u64,
        dma_device_address: u64,
        device_capability: u64,
        mac_address: u64,
    ) -> Self {
        Self {
            magic: NATIVE_NETWORK_BOOT_MAGIC,
            version: NATIVE_NETWORK_BOOT_VERSION,
            size: NATIVE_NETWORK_BOOT_SIZE,
            features: NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK,
            common_config_address,
            receive_notification_address,
            transmit_notification_address,
            dma_virtual_address,
            dma_device_address,
            device_capability,
            mac_address,
            reserved: 0,
        }
    }

    pub const fn is_valid_virtio_network(&self) -> bool {
        self.magic == NATIVE_NETWORK_BOOT_MAGIC
            && self.version == NATIVE_NETWORK_BOOT_VERSION
            && self.size == NATIVE_NETWORK_BOOT_SIZE
            && (self.features == NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
                || self.features
                    == NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES
                || self.features
                    == NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
                        | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES
                        | NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE
                || self.features
                    == NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
                        | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES
                        | NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE
                        | NATIVE_NETWORK_FEATURE_TX_INTERRUPT
                || self.features
                    == NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
                        | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES
                        | NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE
                        | NATIVE_NETWORK_FEATURE_TX_INTERRUPT
                        | NATIVE_NETWORK_FEATURE_ARP_PROBE
                || self.features
                    == NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
                        | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES
                        | NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE
                        | NATIVE_NETWORK_FEATURE_TX_INTERRUPT
                        | NATIVE_NETWORK_FEATURE_ARP_PROBE
                        | NATIVE_NETWORK_FEATURE_RX_REUSE)
            && self.common_config_address != 0
            && self.receive_notification_address != 0
            && self.transmit_notification_address != 0
            && self.receive_notification_address != self.transmit_notification_address
            && self.dma_virtual_address != 0
            && self.dma_device_address != 0
            && self.device_capability != 0
            && self.mac_address != 0
            && self.mac_address >> 48 == 0
            && self.reserved == 0
    }

    pub const fn requests_empty_queues(&self) -> bool {
        self.features & NATIVE_NETWORK_FEATURE_EMPTY_QUEUES != 0
    }

    pub const fn requests_transmit_probe(&self) -> bool {
        self.features & NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE != 0
    }

    pub const fn requests_tx_interrupt(&self) -> bool {
        self.features & NATIVE_NETWORK_FEATURE_TX_INTERRUPT != 0
    }

    pub const fn requests_arp_probe(&self) -> bool {
        self.features & NATIVE_NETWORK_FEATURE_ARP_PROBE != 0
    }

    pub const fn requests_rx_reuse(&self) -> bool {
        self.features & NATIVE_NETWORK_FEATURE_RX_REUSE != 0
    }
}

const _: () = assert!(core::mem::size_of::<NativeNetworkBootV0>() == 80);
const _: () = assert!(core::mem::align_of::<NativeNetworkBootV0>() == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeNetworkBootV1 {
    pub magic: u64,
    pub version: u16,
    pub size: u16,
    pub features: u32,
    pub common_config_address: u64,
    pub receive_notification_address: u64,
    pub transmit_notification_address: u64,
    pub dma_virtual_address: u64,
    pub dma_device_address: u64,
    pub device_capability: u64,
    pub mac_address: u64,
    pub packet_capability: u64,
    pub reserved: u64,
}

impl NativeNetworkBootV1 {
    pub const fn from_v0(v0: NativeNetworkBootV0, packet_capability: u64) -> Self {
        Self {
            magic: v0.magic,
            version: NATIVE_NETWORK_BOOT_V1_VERSION,
            size: NATIVE_NETWORK_BOOT_V1_SIZE,
            features: v0.features | NATIVE_NETWORK_FEATURE_PACKET_HANDOFF,
            common_config_address: v0.common_config_address,
            receive_notification_address: v0.receive_notification_address,
            transmit_notification_address: v0.transmit_notification_address,
            dma_virtual_address: v0.dma_virtual_address,
            dma_device_address: v0.dma_device_address,
            device_capability: v0.device_capability,
            mac_address: v0.mac_address,
            packet_capability,
            reserved: 0,
        }
    }

    pub const fn is_valid_packet_handoff(&self) -> bool {
        self.magic == NATIVE_NETWORK_BOOT_MAGIC
            && self.version == NATIVE_NETWORK_BOOT_V1_VERSION
            && self.size == NATIVE_NETWORK_BOOT_V1_SIZE
            && (self.features
                == NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
                    | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES
                    | NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE
                    | NATIVE_NETWORK_FEATURE_TX_INTERRUPT
                    | NATIVE_NETWORK_FEATURE_ARP_PROBE
                    | NATIVE_NETWORK_FEATURE_RX_REUSE
                    | NATIVE_NETWORK_FEATURE_PACKET_HANDOFF
                || self.features
                    == NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
                        | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES
                        | NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE
                        | NATIVE_NETWORK_FEATURE_TX_INTERRUPT
                        | NATIVE_NETWORK_FEATURE_ARP_PROBE
                        | NATIVE_NETWORK_FEATURE_RX_REUSE
                        | NATIVE_NETWORK_FEATURE_PACKET_HANDOFF
                        | NATIVE_NETWORK_FEATURE_PACKET_FAULT
                || self.features
                    == NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
                        | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES
                        | NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE
                        | NATIVE_NETWORK_FEATURE_TX_INTERRUPT
                        | NATIVE_NETWORK_FEATURE_ARP_PROBE
                        | NATIVE_NETWORK_FEATURE_RX_REUSE
                        | NATIVE_NETWORK_FEATURE_PACKET_HANDOFF
                        | NATIVE_NETWORK_FEATURE_PACKET_BATCH)
            && self.common_config_address != 0
            && self.receive_notification_address != 0
            && self.transmit_notification_address != 0
            && self.receive_notification_address != self.transmit_notification_address
            && self.dma_virtual_address != 0
            && self.dma_device_address != 0
            && self.device_capability != 0
            && self.mac_address != 0
            && self.mac_address >> 48 == 0
            && self.packet_capability != 0
            && self.reserved == 0
    }

    pub const fn v0_prefix(&self) -> NativeNetworkBootV0 {
        NativeNetworkBootV0 {
            magic: self.magic,
            version: NATIVE_NETWORK_BOOT_VERSION,
            size: NATIVE_NETWORK_BOOT_SIZE,
            features: self.features
                & !(NATIVE_NETWORK_FEATURE_PACKET_HANDOFF
                    | NATIVE_NETWORK_FEATURE_PACKET_FAULT
                    | NATIVE_NETWORK_FEATURE_PACKET_BATCH),
            common_config_address: self.common_config_address,
            receive_notification_address: self.receive_notification_address,
            transmit_notification_address: self.transmit_notification_address,
            dma_virtual_address: self.dma_virtual_address,
            dma_device_address: self.dma_device_address,
            device_capability: self.device_capability,
            mac_address: self.mac_address,
            reserved: 0,
        }
    }

    pub const fn requests_packet_fault(&self) -> bool {
        self.features & NATIVE_NETWORK_FEATURE_PACKET_FAULT != 0
    }

    pub const fn requests_packet_batch(&self) -> bool {
        self.features & NATIVE_NETWORK_FEATURE_PACKET_BATCH != 0
    }
}

const _: () = assert!(core::mem::size_of::<NativeNetworkBootV1>() == 88);
const _: () = assert!(core::mem::align_of::<NativeNetworkBootV1>() == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativePacketRequestV0 {
    pub size: u32,
    pub version: u16,
    pub reserved: u16,
    pub buffer_address: u64,
    pub buffer_size: u32,
    pub reserved1: u32,
    pub expected_sequence: u64,
}

impl NativePacketRequestV0 {
    pub const fn publish(buffer_address: u64, buffer_size: u32) -> Self {
        Self {
            size: NATIVE_PACKET_REQUEST_SIZE,
            version: NATIVE_PACKET_REQUEST_VERSION,
            reserved: 0,
            buffer_address,
            buffer_size,
            reserved1: 0,
            expected_sequence: 0,
        }
    }
}

const _: () = assert!(core::mem::size_of::<NativePacketRequestV0>() == 32);
const _: () = assert!(core::mem::align_of::<NativePacketRequestV0>() == 8);

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
        let network_header = include_str!("../../../specs/abi/native-network-bootstrap-v0.h");
        assert!(network_header.contains("UINT64_C(0x30424e58554e4947)"));
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_BOOT_VERSION UINT16_C(0)"));
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_BOOT_SIZE UINT16_C(80)"));
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK UINT32_C(1)"));
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_EMPTY_QUEUES UINT32_C(2)"));
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE UINT32_C(4)"));
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_TX_INTERRUPT UINT32_C(8)"));
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_ARP_PROBE UINT32_C(16)"));
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_RX_REUSE UINT32_C(32)"));
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_PACKET_HANDOFF UINT32_C(64)"));
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_PACKET_FAULT UINT32_C(128)"));
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_PACKET_BATCH UINT32_C(256)"));
        assert!(network_header.contains("GINUX_NATIVE_PACKET_QUEUE_CAPACITY UINT32_C(4)"));
        assert!(network_header.contains("GINUX_NATIVE_PACKET_WAIT_OPERATION UINT64_C(23)"));
        assert!(network_header.contains("GINUX_NATIVE_PACKET_WAIT_TIMEOUT_OPERATION UINT64_C(24)"));
        assert!(network_header.contains("GINUX_NATIVE_PACKET_SPACE_WAIT_OPERATION UINT64_C(25)"));
        assert!(network_header
            .contains("GINUX_NATIVE_PACKET_WAIT_TIMED_OUT (UINT64_MAX - UINT64_C(5))"));
        assert!(network_header.contains("GINUX_NATIVE_PACKET_WAIT_MAX_TIMEOUT_MS UINT64_C(1000)"));
        assert!(network_header.contains("sizeof(struct ginux_native_network_boot_v1) == 88"));
        assert!(network_header.contains("sizeof(struct ginux_native_packet_request_v0) == 32"));
        assert!(network_header.contains("sizeof(struct ginux_native_network_boot_v0) == 80"));
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
        assert!(header.contains("GINUX_NATIVE_NETWORK_DEVICE_OPERATION UINT64_C(16)"));
        assert!(header.contains("GINUX_NATIVE_NETWORK_DEVICE_PROBE UINT64_C(0)"));
        assert!(header.contains("GINUX_NATIVE_NETWORK_DEVICE_ACTIVATE_EMPTY_QUEUES UINT64_C(12)"));
        assert!(header
            .contains("GINUX_NATIVE_NETWORK_DEVICE_DESCRIPTOR_WORD UINT64_C(0x0000000105dc0009)"));
        assert!(header.contains("GINUX_NATIVE_NETWORK_WAIT_OPERATION UINT64_C(20)"));
        assert!(header.contains("GINUX_NATIVE_NETWORK_WAIT_TX_NOTIFIED UINT64_C(1)"));
        assert!(header.contains("GINUX_NATIVE_NETWORK_WAIT_RX_POSTED UINT64_C(2)"));
        assert!(header.contains("GINUX_NATIVE_NETWORK_WAIT_CANCELLED (UINT64_MAX - UINT64_C(5))"));
        assert!(header.contains("GINUX_NATIVE_CONTROL_DISCOVERY_OPERATION UINT64_C(0)"));
        assert!(header.contains("GINUX_NATIVE_CONTROL_ABI_VERSION UINT16_C(4)"));
        assert!(header.contains("GINUX_NATIVE_CONTROL_REQUIRED_FEATURES UINT16_C(1041)"));
        assert!(header.contains("GINUX_NATIVE_CONTROL_DESCRIPTOR_OPERATION UINT64_C(10)"));
        assert!(header.contains("GINUX_NATIVE_CONTROL_FEATURE_PACKET_HANDOFF UINT16_C(2048)"));
        assert!(header.contains("sizeof(struct ginux_native_control_descriptor_v4) == 16"));
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
    fn network_bootstrap_layout_and_validation_are_frozen() {
        assert_eq!(NATIVE_NETWORK_BOOT_MAGIC.to_le_bytes(), *b"GINUXNB0");
        assert_eq!(core::mem::size_of::<NativeNetworkBootV0>(), 80);
        assert_eq!(core::mem::align_of::<NativeNetworkBootV0>(), 8);
        assert_eq!(core::mem::offset_of!(NativeNetworkBootV0, magic), 0);
        assert_eq!(core::mem::offset_of!(NativeNetworkBootV0, version), 8);
        assert_eq!(core::mem::offset_of!(NativeNetworkBootV0, size), 10);
        assert_eq!(core::mem::offset_of!(NativeNetworkBootV0, features), 12);
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV0, common_config_address),
            16
        );
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV0, receive_notification_address),
            24
        );
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV0, transmit_notification_address),
            32
        );
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV0, dma_virtual_address),
            40
        );
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV0, dma_device_address),
            48
        );
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV0, device_capability),
            56
        );
        assert_eq!(core::mem::offset_of!(NativeNetworkBootV0, mac_address), 64);
        assert_eq!(core::mem::offset_of!(NativeNetworkBootV0, reserved), 72);

        let valid = NativeNetworkBootV0::virtio_network(1, 2, 3, 4, 5, 6, 0x5254_0012_3456);
        assert!(valid.is_valid_virtio_network());
        assert!(!valid.requests_empty_queues());
        let active = NativeNetworkBootV0 {
            features: NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES,
            ..valid
        };
        assert!(active.is_valid_virtio_network());
        assert!(active.requests_empty_queues());
        assert!(!active.requests_transmit_probe());
        let transmit = NativeNetworkBootV0 {
            features: active.features | NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE,
            ..active
        };
        assert!(transmit.is_valid_virtio_network());
        assert!(transmit.requests_empty_queues());
        assert!(transmit.requests_transmit_probe());
        assert!(!transmit.requests_tx_interrupt());
        let interrupted = NativeNetworkBootV0 {
            features: transmit.features | NATIVE_NETWORK_FEATURE_TX_INTERRUPT,
            ..transmit
        };
        assert!(interrupted.is_valid_virtio_network());
        assert!(interrupted.requests_tx_interrupt());
        assert!(!interrupted.requests_arp_probe());
        let arp = NativeNetworkBootV0 {
            features: interrupted.features | NATIVE_NETWORK_FEATURE_ARP_PROBE,
            ..interrupted
        };
        assert!(arp.is_valid_virtio_network());
        assert!(arp.requests_arp_probe());
        assert!(!arp.requests_rx_reuse());
        let reused = NativeNetworkBootV0 {
            features: arp.features | NATIVE_NETWORK_FEATURE_RX_REUSE,
            ..arp
        };
        assert!(reused.is_valid_virtio_network());
        assert!(reused.requests_rx_reuse());
        let handoff = NativeNetworkBootV1::from_v0(reused, 7);
        assert!(handoff.is_valid_packet_handoff());
        assert!(!handoff.requests_packet_fault());
        let faulting_handoff = NativeNetworkBootV1 {
            features: handoff.features | NATIVE_NETWORK_FEATURE_PACKET_FAULT,
            ..handoff
        };
        assert!(faulting_handoff.is_valid_packet_handoff());
        assert!(faulting_handoff.requests_packet_fault());
        assert!(!faulting_handoff.requests_packet_batch());
        assert_eq!(faulting_handoff.v0_prefix(), reused);
        let batch_handoff = NativeNetworkBootV1 {
            features: handoff.features | NATIVE_NETWORK_FEATURE_PACKET_BATCH,
            ..handoff
        };
        assert!(batch_handoff.is_valid_packet_handoff());
        assert!(batch_handoff.requests_packet_batch());
        assert!(!batch_handoff.requests_packet_fault());
        assert_eq!(core::mem::size_of::<NativeNetworkBootV1>(), 88);
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV1, packet_capability),
            72
        );
        assert_eq!(core::mem::offset_of!(NativeNetworkBootV1, reserved), 80);
        assert_eq!(handoff.v0_prefix(), reused);
        for invalid in [
            NativeNetworkBootV1 {
                version: 0,
                ..handoff
            },
            NativeNetworkBootV1 {
                size: 80,
                ..handoff
            },
            NativeNetworkBootV1 {
                features: handoff.features & !NATIVE_NETWORK_FEATURE_PACKET_HANDOFF
                    | NATIVE_NETWORK_FEATURE_PACKET_FAULT,
                ..handoff
            },
            NativeNetworkBootV1 {
                packet_capability: 0,
                ..handoff
            },
            NativeNetworkBootV1 {
                reserved: 1,
                ..handoff
            },
        ] {
            assert!(!invalid.is_valid_packet_handoff());
        }
        for invalid in [
            NativeNetworkBootV0 { magic: 0, ..valid },
            NativeNetworkBootV0 {
                version: 1,
                ..valid
            },
            NativeNetworkBootV0 { size: 79, ..valid },
            NativeNetworkBootV0 {
                features: 0,
                ..valid
            },
            NativeNetworkBootV0 {
                features: NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
                    | NATIVE_NETWORK_FEATURE_TX_INTERRUPT,
                ..valid
            },
            NativeNetworkBootV0 {
                features: NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK | NATIVE_NETWORK_FEATURE_RX_REUSE,
                ..valid
            },
            NativeNetworkBootV0 {
                features: NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK | 1 << 6,
                ..valid
            },
            NativeNetworkBootV0 {
                receive_notification_address: valid.transmit_notification_address,
                ..valid
            },
            NativeNetworkBootV0 {
                dma_virtual_address: 0,
                ..valid
            },
            NativeNetworkBootV0 {
                device_capability: 0,
                ..valid
            },
            NativeNetworkBootV0 {
                mac_address: 1 << 48,
                ..valid
            },
            NativeNetworkBootV0 {
                reserved: 1,
                ..valid
            },
        ] {
            assert!(!invalid.is_valid_virtio_network());
        }
    }

    #[test]
    fn control_discovery_requires_v4_and_required_features() {
        let compatible = (u64::from(NATIVE_CONTROL_ABI_VERSION) << 16)
            | u64::from(NATIVE_CONTROL_REQUIRED_FEATURES);
        assert!(native_control_abi_is_compatible(compatible));
        assert!(!native_control_abi_supports_packet_handoff(compatible));
        assert!(native_control_abi_supports_packet_handoff(
            compatible | u64::from(NATIVE_CONTROL_FEATURE_PACKET_HANDOFF)
        ));
        assert!(native_control_abi_is_compatible(
            compatible | u64::from(1_u16 << 15)
        ));
        assert!(!native_control_abi_is_compatible(compatible & !1));
        assert!(!native_control_abi_is_compatible(compatible & !(1 << 4)));
        assert!(!native_control_abi_is_compatible(compatible & !(1 << 10)));
        assert!(!native_control_abi_is_compatible(compatible + (1 << 16)));
        assert!(!native_control_abi_is_compatible(
            compatible | (1_u64 << 32)
        ));
    }

    #[test]
    fn control_descriptor_v4_layout_and_validation_are_frozen() {
        let valid = NativeControlDescriptorV4 {
            size: NATIVE_CONTROL_DESCRIPTOR_SIZE,
            version: NATIVE_CONTROL_ABI_VERSION,
            features: NATIVE_CONTROL_REQUIRED_FEATURES,
            max_buffered_console_bytes: NATIVE_CONTROL_MAX_BUFFERED_CONSOLE_BYTES,
            reserved: 0,
        };
        assert_eq!(core::mem::size_of::<NativeControlDescriptorV4>(), 16);
        assert_eq!(core::mem::offset_of!(NativeControlDescriptorV4, size), 0);
        assert_eq!(core::mem::offset_of!(NativeControlDescriptorV4, version), 4);
        assert_eq!(
            core::mem::offset_of!(NativeControlDescriptorV4, features),
            6
        );
        assert_eq!(
            core::mem::offset_of!(NativeControlDescriptorV4, max_buffered_console_bytes),
            8
        );
        assert_eq!(
            core::mem::offset_of!(NativeControlDescriptorV4, reserved),
            12
        );
        assert!(valid.is_compatible());
        assert!(!NativeControlDescriptorV4 { size: 15, ..valid }.is_compatible());
        assert!(!NativeControlDescriptorV4 {
            version: 3,
            ..valid
        }
        .is_compatible());
        assert!(!NativeControlDescriptorV4 {
            features: valid.features & !(1 << 10),
            ..valid
        }
        .is_compatible());
        assert!(!NativeControlDescriptorV4 {
            reserved: 1,
            ..valid
        }
        .is_compatible());
    }
}

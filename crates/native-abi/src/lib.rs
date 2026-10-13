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
pub const NATIVE_NETWORK_FEATURE_PACKET_EGRESS: u32 = 1 << 9;
pub const NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS: u32 = 1 << 10;
pub const NATIVE_NETWORK_FEATURE_EGRESS_FAULT: u32 = 1 << 11;
pub const NATIVE_NETWORK_FEATURE_EGRESS_WAIT: u32 = 1 << 12;
pub const NATIVE_NETWORK_FEATURE_EGRESS_REUSE: u32 = 1 << 13;
pub const NATIVE_NETWORK_FEATURE_EGRESS_BATCH: u32 = 1 << 14;
pub const NATIVE_NETWORK_FEATURE_EGRESS_RESIDENT: u32 = 1 << 15;
pub const NATIVE_NETWORK_BOOT_V1_VERSION: u16 = 1;
pub const NATIVE_NETWORK_BOOT_V1_SIZE: u16 = 88;
pub const NATIVE_NETWORK_BOOT_V2_VERSION: u16 = 2;
pub const NATIVE_NETWORK_BOOT_V2_SIZE: u16 = 96;
pub const NATIVE_NETWORK_BOOT_V3_VERSION: u16 = 3;
pub const NATIVE_NETWORK_BOOT_V3_SIZE: u16 = 96;
pub const NATIVE_NETWORK_BOOT_V4_VERSION: u16 = 4;
pub const NATIVE_NETWORK_BOOT_V4_SIZE: u16 = 96;
pub const NATIVE_NETWORK_BOOT_V5_VERSION: u16 = 5;
pub const NATIVE_NETWORK_BOOT_V5_SIZE: u16 = 96;
pub const NATIVE_NETWORK_BOOT_V6_VERSION: u16 = 6;
pub const NATIVE_NETWORK_BOOT_V6_SIZE: u16 = 96;
pub const NATIVE_NETWORK_BOOT_V7_VERSION: u16 = 7;
pub const NATIVE_NETWORK_BOOT_V7_SIZE: u16 = 96;
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
pub const NATIVE_PACKET_EGRESS_CLAIM_OPERATION: u64 = 26;
pub const NATIVE_PACKET_EGRESS_ACK_OPERATION: u64 = 27;
pub const NATIVE_PACKET_EGRESS_WAIT_OPERATION: u64 = 28;
pub const NATIVE_PACKET_EGRESS_SPACE_WAIT_OPERATION: u64 = 29;
pub const NATIVE_PACKET_EGRESS_DRAIN_WAIT_OPERATION: u64 = 30;
pub const NATIVE_PACKET_REQUEST_VERSION: u16 = 0;
pub const NATIVE_PACKET_REQUEST_SIZE: u32 = 32;
pub const NATIVE_PACKET_MAX_SIZE: u32 = 1514;
pub const NATIVE_PACKET_QUEUE_CAPACITY: u32 = 4;
pub const NATIVE_PACKET_WAIT_TIMED_OUT: u64 = u64::MAX - 5;
pub const NATIVE_PACKET_DRAINED: u64 = u64::MAX - 6;
pub const NATIVE_PACKET_WAIT_MAX_TIMEOUT_MS: u64 = 1_000;
pub const NATIVE_PACKET_STALE: u64 = u64::MAX - 4;
pub const NATIVE_CALL_FAILED: u64 = u64::MAX - 2;
pub const NATIVE_PROTOCOL_BOOT_MAGIC: u64 = 0x3042_5058_554e_4947;
pub const NATIVE_PROTOCOL_BOOT_VERSION: u16 = 0;
pub const NATIVE_PROTOCOL_BOOT_SIZE: u16 = 56;
pub const NATIVE_PROTOCOL_FEATURE_PACKET_CONSUMER: u32 = 1 << 0;
pub const NATIVE_PROTOCOL_FEATURE_ETHERNET_ARP: u32 = 1 << 1;
pub const NATIVE_PROTOCOL_FEATURE_WAIT_LOOP: u32 = 1 << 2;
pub const NATIVE_PROTOCOL_FEATURE_SUPERVISED_LOOP: u32 = 1 << 3;
pub const NATIVE_PROTOCOL_FEATURE_IPV4_DISPATCH: u32 = 1 << 4;
pub const NATIVE_PROTOCOL_FEATURE_ICMP_ECHO: u32 = 1 << 5;
pub const NATIVE_PROTOCOL_FEATURE_PACKET_EGRESS: u32 = 1 << 6;
pub const NATIVE_PROTOCOL_BOOT_V1_VERSION: u16 = 1;
pub const NATIVE_PROTOCOL_BOOT_V1_SIZE: u16 = 56;
pub const NATIVE_PROTOCOL_BOOT_V2_VERSION: u16 = 2;
pub const NATIVE_PROTOCOL_BOOT_V2_SIZE: u16 = 56;
pub const NATIVE_PROTOCOL_BOOT_V3_VERSION: u16 = 3;
pub const NATIVE_PROTOCOL_BOOT_V3_SIZE: u16 = 56;
pub const NATIVE_PROTOCOL_BOOT_V4_VERSION: u16 = 4;
pub const NATIVE_PROTOCOL_BOOT_V4_SIZE: u16 = 56;
pub const NATIVE_PROTOCOL_BOOT_V5_VERSION: u16 = 5;
pub const NATIVE_PROTOCOL_BOOT_V5_SIZE: u16 = 72;
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
pub const NATIVE_CONSOLE_WRITE_OPERATION: u64 = 9;
pub const NATIVE_CONSOLE_READ_LINE_OPERATION: u64 = 31;
pub const NATIVE_CONSOLE_MAX_LINE_BYTES: u32 = 32;
pub const NATIVE_HOST_INSPECT_OPERATION: u64 = 32;
pub const NATIVE_HOST_INSPECT_REQUEST_SIZE: u32 = 32;
pub const NATIVE_HOST_INSPECT_RECORD_SIZE: u32 = 40;
pub const NATIVE_HOST_INSPECT_VERSION: u16 = 0;
pub const NATIVE_HOST_PROC_ENUMERATE: u16 = 1;
pub const NATIVE_HOST_PROC_STATUS: u16 = 2;
pub const NATIVE_HOST_PROCESS_KIND_CONTROLLER: u16 = 1;
pub const NATIVE_HOST_INSPECT_STATE_READY: u16 = 1;
pub const NATIVE_HOST_INSPECT_STATE_STOPPING: u16 = 2;
pub const NATIVE_ROOT_OPERATION: u64 = 17;
pub const NATIVE_ROOT_READ_DIRECTORY: u64 = 3;
pub const NATIVE_ROOT_DIRECTORY_REQUEST_SIZE: u32 = 48;
pub const NATIVE_ROOT_DIRECTORY_RECORD_HEADER_SIZE: usize = 32;
pub const NATIVE_ROOT_MAX_DIRECTORY_RECORD_SIZE: usize = 287;

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
#[derive(Clone, Copy)]
pub struct NativeRootDirectoryRequestV4 {
    pub size: u32,
    pub version: u16,
    pub reserved: u16,
    pub path_address: u64,
    pub path_size: u64,
    pub cursor: u64,
    pub output_address: u64,
    pub output_capacity: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeHostInspectRequestV0 {
    pub size: u32,
    pub version: u16,
    pub command: u16,
    pub cursor: u64,
    pub output_address: u64,
    pub output_size: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeHostInspectRecordV0 {
    pub size: u32,
    pub version: u16,
    pub kind: u16,
    pub state: u16,
    pub reserved: u16,
    pub active_threads: u32,
    pub pid: u64,
    pub parent_pid: u64,
    pub next_cursor: u64,
}

impl NativeHostInspectRecordV0 {
    pub const fn zeroed() -> Self {
        Self {
            size: 0,
            version: 0,
            kind: 0,
            state: 0,
            reserved: 0,
            active_threads: 0,
            pid: 0,
            parent_pid: 0,
            next_cursor: 0,
        }
    }
}

const _: () = assert!(core::mem::size_of::<NativeRootDirectoryRequestV4>() == 48);
const _: () = assert!(core::mem::size_of::<NativeHostInspectRequestV0>() == 32);
const _: () = assert!(core::mem::size_of::<NativeHostInspectRecordV0>() == 40);

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
pub struct NativeNetworkBootV2 {
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
    pub egress_capability: u64,
    pub expected_egress_sequence: u64,
    pub reserved: u64,
}

impl NativeNetworkBootV2 {
    pub const fn egress_transmit(
        v0: NativeNetworkBootV0,
        egress_capability: u64,
        expected_egress_sequence: u64,
    ) -> Self {
        Self {
            magic: v0.magic,
            version: NATIVE_NETWORK_BOOT_V2_VERSION,
            size: NATIVE_NETWORK_BOOT_V2_SIZE,
            features: v0.features | NATIVE_NETWORK_FEATURE_PACKET_EGRESS,
            common_config_address: v0.common_config_address,
            receive_notification_address: v0.receive_notification_address,
            transmit_notification_address: v0.transmit_notification_address,
            dma_virtual_address: v0.dma_virtual_address,
            dma_device_address: v0.dma_device_address,
            device_capability: v0.device_capability,
            mac_address: v0.mac_address,
            egress_capability,
            expected_egress_sequence,
            reserved: 0,
        }
    }

    pub const fn is_valid_egress_transmit(&self) -> bool {
        self.magic == NATIVE_NETWORK_BOOT_MAGIC
            && self.version == NATIVE_NETWORK_BOOT_V2_VERSION
            && self.size == NATIVE_NETWORK_BOOT_V2_SIZE
            && self.features
                == NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
                    | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES
                    | NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE
                    | NATIVE_NETWORK_FEATURE_TX_INTERRUPT
                    | NATIVE_NETWORK_FEATURE_PACKET_EGRESS
            && self.common_config_address != 0
            && self.receive_notification_address != 0
            && self.transmit_notification_address != 0
            && self.receive_notification_address != self.transmit_notification_address
            && self.dma_virtual_address != 0
            && self.dma_device_address != 0
            && self.device_capability != 0
            && self.mac_address != 0
            && self.mac_address >> 48 == 0
            && self.egress_capability != 0
            && self.egress_capability != self.device_capability
            && self.expected_egress_sequence != 0
            && self.expected_egress_sequence != u64::MAX
            && self.reserved == 0
    }

    pub const fn v0_prefix(&self) -> NativeNetworkBootV0 {
        NativeNetworkBootV0 {
            magic: self.magic,
            version: NATIVE_NETWORK_BOOT_VERSION,
            size: NATIVE_NETWORK_BOOT_SIZE,
            features: self.features & !NATIVE_NETWORK_FEATURE_PACKET_EGRESS,
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
}

const _: () = assert!(core::mem::size_of::<NativeNetworkBootV2>() == 96);
const _: () = assert!(core::mem::align_of::<NativeNetworkBootV2>() == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeNetworkBootV3 {
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
    pub egress_capability: u64,
    pub expected_egress_sequence: u64,
    pub reserved: u64,
}

impl NativeNetworkBootV3 {
    pub const fn reliable_egress_transmit(
        v0: NativeNetworkBootV0,
        egress_capability: u64,
        expected_egress_sequence: u64,
        fault_after_claim: bool,
    ) -> Self {
        Self {
            magic: v0.magic,
            version: NATIVE_NETWORK_BOOT_V3_VERSION,
            size: NATIVE_NETWORK_BOOT_V3_SIZE,
            features: v0.features
                | NATIVE_NETWORK_FEATURE_PACKET_EGRESS
                | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS
                | if fault_after_claim {
                    NATIVE_NETWORK_FEATURE_EGRESS_FAULT
                } else {
                    0
                },
            common_config_address: v0.common_config_address,
            receive_notification_address: v0.receive_notification_address,
            transmit_notification_address: v0.transmit_notification_address,
            dma_virtual_address: v0.dma_virtual_address,
            dma_device_address: v0.dma_device_address,
            device_capability: v0.device_capability,
            mac_address: v0.mac_address,
            egress_capability,
            expected_egress_sequence,
            reserved: 0,
        }
    }

    pub const fn is_valid_reliable_egress_transmit(&self) -> bool {
        let base = NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
            | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES
            | NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE
            | NATIVE_NETWORK_FEATURE_TX_INTERRUPT
            | NATIVE_NETWORK_FEATURE_PACKET_EGRESS
            | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS;
        self.magic == NATIVE_NETWORK_BOOT_MAGIC
            && self.version == NATIVE_NETWORK_BOOT_V3_VERSION
            && self.size == NATIVE_NETWORK_BOOT_V3_SIZE
            && (self.features == base
                || self.features == base | NATIVE_NETWORK_FEATURE_EGRESS_FAULT)
            && self.common_config_address != 0
            && self.receive_notification_address != 0
            && self.transmit_notification_address != 0
            && self.receive_notification_address != self.transmit_notification_address
            && self.dma_virtual_address != 0
            && self.dma_device_address != 0
            && self.device_capability != 0
            && self.mac_address != 0
            && self.mac_address >> 48 == 0
            && self.egress_capability != 0
            && self.egress_capability != self.device_capability
            && self.expected_egress_sequence != 0
            && self.expected_egress_sequence != u64::MAX
            && self.reserved == 0
    }

    pub const fn requests_fault_after_claim(&self) -> bool {
        self.features & NATIVE_NETWORK_FEATURE_EGRESS_FAULT != 0
    }

    pub const fn v0_prefix(&self) -> NativeNetworkBootV0 {
        NativeNetworkBootV0 {
            magic: self.magic,
            version: NATIVE_NETWORK_BOOT_VERSION,
            size: NATIVE_NETWORK_BOOT_SIZE,
            features: self.features
                & !(NATIVE_NETWORK_FEATURE_PACKET_EGRESS
                    | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS
                    | NATIVE_NETWORK_FEATURE_EGRESS_FAULT),
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
}

const _: () = assert!(core::mem::size_of::<NativeNetworkBootV3>() == 96);
const _: () = assert!(core::mem::align_of::<NativeNetworkBootV3>() == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeNetworkBootV4 {
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
    pub egress_capability: u64,
    pub expected_egress_sequence: u64,
    pub reserved: u64,
}

impl NativeNetworkBootV4 {
    pub const fn waiting_reliable_egress_transmit(
        v0: NativeNetworkBootV0,
        egress_capability: u64,
        expected_egress_sequence: u64,
    ) -> Self {
        Self {
            magic: v0.magic,
            version: NATIVE_NETWORK_BOOT_V4_VERSION,
            size: NATIVE_NETWORK_BOOT_V4_SIZE,
            features: v0.features
                | NATIVE_NETWORK_FEATURE_PACKET_EGRESS
                | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS
                | NATIVE_NETWORK_FEATURE_EGRESS_WAIT,
            common_config_address: v0.common_config_address,
            receive_notification_address: v0.receive_notification_address,
            transmit_notification_address: v0.transmit_notification_address,
            dma_virtual_address: v0.dma_virtual_address,
            dma_device_address: v0.dma_device_address,
            device_capability: v0.device_capability,
            mac_address: v0.mac_address,
            egress_capability,
            expected_egress_sequence,
            reserved: 0,
        }
    }

    pub const fn is_valid_waiting_reliable_egress_transmit(&self) -> bool {
        self.magic == NATIVE_NETWORK_BOOT_MAGIC
            && self.version == NATIVE_NETWORK_BOOT_V4_VERSION
            && self.size == NATIVE_NETWORK_BOOT_V4_SIZE
            && self.features
                == NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
                    | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES
                    | NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE
                    | NATIVE_NETWORK_FEATURE_TX_INTERRUPT
                    | NATIVE_NETWORK_FEATURE_PACKET_EGRESS
                    | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS
                    | NATIVE_NETWORK_FEATURE_EGRESS_WAIT
            && self.common_config_address != 0
            && self.receive_notification_address != 0
            && self.transmit_notification_address != 0
            && self.receive_notification_address != self.transmit_notification_address
            && self.dma_virtual_address != 0
            && self.dma_device_address != 0
            && self.device_capability != 0
            && self.mac_address != 0
            && self.mac_address >> 48 == 0
            && self.egress_capability != 0
            && self.egress_capability != self.device_capability
            && self.expected_egress_sequence != 0
            && self.expected_egress_sequence != u64::MAX
            && self.reserved == 0
    }

    pub const fn v0_prefix(&self) -> NativeNetworkBootV0 {
        NativeNetworkBootV0 {
            magic: self.magic,
            version: NATIVE_NETWORK_BOOT_VERSION,
            size: NATIVE_NETWORK_BOOT_SIZE,
            features: self.features
                & !(NATIVE_NETWORK_FEATURE_PACKET_EGRESS
                    | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS
                    | NATIVE_NETWORK_FEATURE_EGRESS_WAIT),
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
}

const _: () = assert!(core::mem::size_of::<NativeNetworkBootV4>() == 96);
const _: () = assert!(core::mem::align_of::<NativeNetworkBootV4>() == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeNetworkBootV5 {
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
    pub egress_capability: u64,
    pub expected_egress_sequence: u64,
    pub reserved: u64,
}

impl NativeNetworkBootV5 {
    pub const fn resident_reliable_egress_transmit(
        v0: NativeNetworkBootV0,
        egress_capability: u64,
        expected_egress_sequence: u64,
    ) -> Self {
        Self {
            magic: v0.magic,
            version: NATIVE_NETWORK_BOOT_V5_VERSION,
            size: NATIVE_NETWORK_BOOT_V5_SIZE,
            features: v0.features
                | NATIVE_NETWORK_FEATURE_PACKET_EGRESS
                | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS
                | NATIVE_NETWORK_FEATURE_EGRESS_WAIT
                | NATIVE_NETWORK_FEATURE_EGRESS_REUSE,
            common_config_address: v0.common_config_address,
            receive_notification_address: v0.receive_notification_address,
            transmit_notification_address: v0.transmit_notification_address,
            dma_virtual_address: v0.dma_virtual_address,
            dma_device_address: v0.dma_device_address,
            device_capability: v0.device_capability,
            mac_address: v0.mac_address,
            egress_capability,
            expected_egress_sequence,
            reserved: 0,
        }
    }

    pub const fn is_valid_resident_reliable_egress_transmit(&self) -> bool {
        self.magic == NATIVE_NETWORK_BOOT_MAGIC
            && self.version == NATIVE_NETWORK_BOOT_V5_VERSION
            && self.size == NATIVE_NETWORK_BOOT_V5_SIZE
            && self.features
                == NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
                    | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES
                    | NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE
                    | NATIVE_NETWORK_FEATURE_TX_INTERRUPT
                    | NATIVE_NETWORK_FEATURE_PACKET_EGRESS
                    | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS
                    | NATIVE_NETWORK_FEATURE_EGRESS_WAIT
                    | NATIVE_NETWORK_FEATURE_EGRESS_REUSE
            && self.common_config_address != 0
            && self.receive_notification_address != 0
            && self.transmit_notification_address != 0
            && self.receive_notification_address != self.transmit_notification_address
            && self.dma_virtual_address != 0
            && self.dma_device_address != 0
            && self.device_capability != 0
            && self.mac_address != 0
            && self.mac_address >> 48 == 0
            && self.egress_capability != 0
            && self.egress_capability != self.device_capability
            && self.expected_egress_sequence != 0
            && self.expected_egress_sequence.checked_add(1).is_some()
            && self.reserved == 0
    }

    pub const fn v0_prefix(&self) -> NativeNetworkBootV0 {
        NativeNetworkBootV0 {
            magic: self.magic,
            version: NATIVE_NETWORK_BOOT_VERSION,
            size: NATIVE_NETWORK_BOOT_SIZE,
            features: self.features
                & !(NATIVE_NETWORK_FEATURE_PACKET_EGRESS
                    | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS
                    | NATIVE_NETWORK_FEATURE_EGRESS_WAIT
                    | NATIVE_NETWORK_FEATURE_EGRESS_REUSE),
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
}

const _: () = assert!(core::mem::size_of::<NativeNetworkBootV5>() == 96);
const _: () = assert!(core::mem::align_of::<NativeNetworkBootV5>() == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeNetworkBootV6 {
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
    pub egress_capability: u64,
    pub expected_egress_sequence: u64,
    pub reserved: u64,
}

impl NativeNetworkBootV6 {
    pub const fn resident_reliable_egress_batch(
        v0: NativeNetworkBootV0,
        egress_capability: u64,
        expected_egress_sequence: u64,
    ) -> Self {
        Self {
            magic: v0.magic,
            version: NATIVE_NETWORK_BOOT_V6_VERSION,
            size: NATIVE_NETWORK_BOOT_V6_SIZE,
            features: v0.features
                | NATIVE_NETWORK_FEATURE_PACKET_EGRESS
                | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS
                | NATIVE_NETWORK_FEATURE_EGRESS_WAIT
                | NATIVE_NETWORK_FEATURE_EGRESS_REUSE
                | NATIVE_NETWORK_FEATURE_EGRESS_BATCH,
            common_config_address: v0.common_config_address,
            receive_notification_address: v0.receive_notification_address,
            transmit_notification_address: v0.transmit_notification_address,
            dma_virtual_address: v0.dma_virtual_address,
            dma_device_address: v0.dma_device_address,
            device_capability: v0.device_capability,
            mac_address: v0.mac_address,
            egress_capability,
            expected_egress_sequence,
            reserved: 0,
        }
    }

    pub const fn is_valid_resident_reliable_egress_batch(&self) -> bool {
        self.magic == NATIVE_NETWORK_BOOT_MAGIC
            && self.version == NATIVE_NETWORK_BOOT_V6_VERSION
            && self.size == NATIVE_NETWORK_BOOT_V6_SIZE
            && self.features
                == NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
                    | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES
                    | NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE
                    | NATIVE_NETWORK_FEATURE_TX_INTERRUPT
                    | NATIVE_NETWORK_FEATURE_PACKET_EGRESS
                    | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS
                    | NATIVE_NETWORK_FEATURE_EGRESS_WAIT
                    | NATIVE_NETWORK_FEATURE_EGRESS_REUSE
                    | NATIVE_NETWORK_FEATURE_EGRESS_BATCH
            && self.common_config_address != 0
            && self.receive_notification_address != 0
            && self.transmit_notification_address != 0
            && self.receive_notification_address != self.transmit_notification_address
            && self.dma_virtual_address != 0
            && self.dma_device_address != 0
            && self.device_capability != 0
            && self.mac_address != 0
            && self.mac_address >> 48 == 0
            && self.egress_capability != 0
            && self.egress_capability != self.device_capability
            && self.expected_egress_sequence != 0
            && self.expected_egress_sequence.checked_add(3).is_some()
            && self.reserved == 0
    }

    pub const fn v0_prefix(&self) -> NativeNetworkBootV0 {
        NativeNetworkBootV0 {
            magic: self.magic,
            version: NATIVE_NETWORK_BOOT_VERSION,
            size: NATIVE_NETWORK_BOOT_SIZE,
            features: self.features
                & !(NATIVE_NETWORK_FEATURE_PACKET_EGRESS
                    | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS
                    | NATIVE_NETWORK_FEATURE_EGRESS_WAIT
                    | NATIVE_NETWORK_FEATURE_EGRESS_REUSE
                    | NATIVE_NETWORK_FEATURE_EGRESS_BATCH),
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
}

const _: () = assert!(core::mem::size_of::<NativeNetworkBootV6>() == 96);
const _: () = assert!(core::mem::align_of::<NativeNetworkBootV6>() == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeNetworkBootV7 {
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
    pub egress_capability: u64,
    pub expected_egress_sequence: u64,
    pub reserved: u64,
}

impl NativeNetworkBootV7 {
    pub const fn resident_reliable_egress_loop(
        v0: NativeNetworkBootV0,
        egress_capability: u64,
        expected_egress_sequence: u64,
    ) -> Self {
        Self {
            magic: v0.magic,
            version: NATIVE_NETWORK_BOOT_V7_VERSION,
            size: NATIVE_NETWORK_BOOT_V7_SIZE,
            features: v0.features
                | NATIVE_NETWORK_FEATURE_PACKET_EGRESS
                | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS
                | NATIVE_NETWORK_FEATURE_EGRESS_WAIT
                | NATIVE_NETWORK_FEATURE_EGRESS_REUSE
                | NATIVE_NETWORK_FEATURE_EGRESS_BATCH
                | NATIVE_NETWORK_FEATURE_EGRESS_RESIDENT,
            common_config_address: v0.common_config_address,
            receive_notification_address: v0.receive_notification_address,
            transmit_notification_address: v0.transmit_notification_address,
            dma_virtual_address: v0.dma_virtual_address,
            dma_device_address: v0.dma_device_address,
            device_capability: v0.device_capability,
            mac_address: v0.mac_address,
            egress_capability,
            expected_egress_sequence,
            reserved: 0,
        }
    }

    pub const fn is_valid_resident_reliable_egress_loop(&self) -> bool {
        self.magic == NATIVE_NETWORK_BOOT_MAGIC
            && self.version == NATIVE_NETWORK_BOOT_V7_VERSION
            && self.size == NATIVE_NETWORK_BOOT_V7_SIZE
            && self.features
                == NATIVE_NETWORK_FEATURE_VIRTIO_NETWORK
                    | NATIVE_NETWORK_FEATURE_EMPTY_QUEUES
                    | NATIVE_NETWORK_FEATURE_TRANSMIT_PROBE
                    | NATIVE_NETWORK_FEATURE_TX_INTERRUPT
                    | NATIVE_NETWORK_FEATURE_PACKET_EGRESS
                    | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS
                    | NATIVE_NETWORK_FEATURE_EGRESS_WAIT
                    | NATIVE_NETWORK_FEATURE_EGRESS_REUSE
                    | NATIVE_NETWORK_FEATURE_EGRESS_BATCH
                    | NATIVE_NETWORK_FEATURE_EGRESS_RESIDENT
            && self.common_config_address != 0
            && self.receive_notification_address != 0
            && self.transmit_notification_address != 0
            && self.receive_notification_address != self.transmit_notification_address
            && self.dma_virtual_address != 0
            && self.dma_device_address != 0
            && self.device_capability != 0
            && self.mac_address != 0
            && self.mac_address >> 48 == 0
            && self.egress_capability != 0
            && self.egress_capability != self.device_capability
            && self.expected_egress_sequence != 0
            && self.expected_egress_sequence.checked_add(4).is_some()
            && self.reserved == 0
    }

    pub const fn v0_prefix(&self) -> NativeNetworkBootV0 {
        NativeNetworkBootV0 {
            magic: self.magic,
            version: NATIVE_NETWORK_BOOT_VERSION,
            size: NATIVE_NETWORK_BOOT_SIZE,
            features: self.features
                & !(NATIVE_NETWORK_FEATURE_PACKET_EGRESS
                    | NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS
                    | NATIVE_NETWORK_FEATURE_EGRESS_WAIT
                    | NATIVE_NETWORK_FEATURE_EGRESS_REUSE
                    | NATIVE_NETWORK_FEATURE_EGRESS_BATCH
                    | NATIVE_NETWORK_FEATURE_EGRESS_RESIDENT),
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
}

const _: () = assert!(core::mem::size_of::<NativeNetworkBootV7>() == 96);
const _: () = assert!(core::mem::align_of::<NativeNetworkBootV7>() == 8);

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

    pub const fn consume(buffer_address: u64, buffer_size: u32, expected_sequence: u64) -> Self {
        Self {
            size: NATIVE_PACKET_REQUEST_SIZE,
            version: NATIVE_PACKET_REQUEST_VERSION,
            reserved: 0,
            buffer_address,
            buffer_size,
            reserved1: 0,
            expected_sequence,
        }
    }
}

const _: () = assert!(core::mem::size_of::<NativePacketRequestV0>() == 32);
const _: () = assert!(core::mem::align_of::<NativePacketRequestV0>() == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeProtocolBootV0 {
    pub magic: u64,
    pub version: u16,
    pub size: u16,
    pub features: u32,
    pub packet_capability: u64,
    pub expected_sequence: u64,
    pub local_mac: u64,
    pub local_ipv4: u32,
    pub peer_ipv4: u32,
    pub reserved: u64,
}

impl NativeProtocolBootV0 {
    pub const fn arp_consumer(
        packet_capability: u64,
        expected_sequence: u64,
        local_mac: u64,
        local_ipv4: u32,
        peer_ipv4: u32,
    ) -> Self {
        Self {
            magic: NATIVE_PROTOCOL_BOOT_MAGIC,
            version: NATIVE_PROTOCOL_BOOT_VERSION,
            size: NATIVE_PROTOCOL_BOOT_SIZE,
            features: NATIVE_PROTOCOL_FEATURE_PACKET_CONSUMER
                | NATIVE_PROTOCOL_FEATURE_ETHERNET_ARP,
            packet_capability,
            expected_sequence,
            local_mac,
            local_ipv4,
            peer_ipv4,
            reserved: 0,
        }
    }

    pub const fn is_valid_arp_consumer(&self) -> bool {
        self.magic == NATIVE_PROTOCOL_BOOT_MAGIC
            && self.version == NATIVE_PROTOCOL_BOOT_VERSION
            && self.size == NATIVE_PROTOCOL_BOOT_SIZE
            && self.features
                == NATIVE_PROTOCOL_FEATURE_PACKET_CONSUMER | NATIVE_PROTOCOL_FEATURE_ETHERNET_ARP
            && self.packet_capability != 0
            && self.expected_sequence != 0
            && self.expected_sequence != u64::MAX
            && self.local_mac != 0
            && self.local_mac >> 48 == 0
            && self.local_ipv4 != 0
            && self.peer_ipv4 != 0
            && self.local_ipv4 != self.peer_ipv4
            && self.reserved == 0
    }
}

const _: () = assert!(core::mem::size_of::<NativeProtocolBootV0>() == 56);
const _: () = assert!(core::mem::align_of::<NativeProtocolBootV0>() == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeProtocolBootV1 {
    pub magic: u64,
    pub version: u16,
    pub size: u16,
    pub features: u32,
    pub packet_capability: u64,
    pub expected_sequence: u64,
    pub local_mac: u64,
    pub local_ipv4: u32,
    pub peer_ipv4: u32,
    pub packet_count: u32,
    pub reserved: u32,
}

impl NativeProtocolBootV1 {
    pub const fn wait_loop(v0: NativeProtocolBootV0, packet_count: u32) -> Self {
        Self {
            magic: v0.magic,
            version: NATIVE_PROTOCOL_BOOT_V1_VERSION,
            size: NATIVE_PROTOCOL_BOOT_V1_SIZE,
            features: NATIVE_PROTOCOL_FEATURE_PACKET_CONSUMER
                | NATIVE_PROTOCOL_FEATURE_ETHERNET_ARP
                | NATIVE_PROTOCOL_FEATURE_WAIT_LOOP,
            packet_capability: v0.packet_capability,
            expected_sequence: v0.expected_sequence,
            local_mac: v0.local_mac,
            local_ipv4: v0.local_ipv4,
            peer_ipv4: v0.peer_ipv4,
            packet_count,
            reserved: 0,
        }
    }

    pub const fn is_valid_wait_loop(&self) -> bool {
        self.magic == NATIVE_PROTOCOL_BOOT_MAGIC
            && self.version == NATIVE_PROTOCOL_BOOT_V1_VERSION
            && self.size == NATIVE_PROTOCOL_BOOT_V1_SIZE
            && self.features
                == NATIVE_PROTOCOL_FEATURE_PACKET_CONSUMER
                    | NATIVE_PROTOCOL_FEATURE_ETHERNET_ARP
                    | NATIVE_PROTOCOL_FEATURE_WAIT_LOOP
            && self.packet_capability != 0
            && self.expected_sequence != 0
            && self.expected_sequence < u64::MAX - 1
            && self.local_mac != 0
            && self.local_mac >> 48 == 0
            && self.local_ipv4 != 0
            && self.peer_ipv4 != 0
            && self.local_ipv4 != self.peer_ipv4
            && self.packet_count == 2
            && self.reserved == 0
    }
}

const _: () = assert!(core::mem::size_of::<NativeProtocolBootV1>() == 56);
const _: () = assert!(core::mem::align_of::<NativeProtocolBootV1>() == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeProtocolBootV2 {
    pub magic: u64,
    pub version: u16,
    pub size: u16,
    pub features: u32,
    pub packet_capability: u64,
    pub expected_sequence: u64,
    pub local_mac: u64,
    pub local_ipv4: u32,
    pub peer_ipv4: u32,
    pub packet_count: u32,
    pub reserved: u32,
}

impl NativeProtocolBootV2 {
    pub const fn supervised_loop(v0: NativeProtocolBootV0) -> Self {
        Self {
            magic: v0.magic,
            version: NATIVE_PROTOCOL_BOOT_V2_VERSION,
            size: NATIVE_PROTOCOL_BOOT_V2_SIZE,
            features: NATIVE_PROTOCOL_FEATURE_PACKET_CONSUMER
                | NATIVE_PROTOCOL_FEATURE_ETHERNET_ARP
                | NATIVE_PROTOCOL_FEATURE_WAIT_LOOP
                | NATIVE_PROTOCOL_FEATURE_SUPERVISED_LOOP,
            packet_capability: v0.packet_capability,
            expected_sequence: v0.expected_sequence,
            local_mac: v0.local_mac,
            local_ipv4: v0.local_ipv4,
            peer_ipv4: v0.peer_ipv4,
            packet_count: 0,
            reserved: 0,
        }
    }

    pub const fn is_valid_supervised_loop(&self) -> bool {
        self.magic == NATIVE_PROTOCOL_BOOT_MAGIC
            && self.version == NATIVE_PROTOCOL_BOOT_V2_VERSION
            && self.size == NATIVE_PROTOCOL_BOOT_V2_SIZE
            && self.features
                == NATIVE_PROTOCOL_FEATURE_PACKET_CONSUMER
                    | NATIVE_PROTOCOL_FEATURE_ETHERNET_ARP
                    | NATIVE_PROTOCOL_FEATURE_WAIT_LOOP
                    | NATIVE_PROTOCOL_FEATURE_SUPERVISED_LOOP
            && self.packet_capability != 0
            && self.expected_sequence != 0
            && self.expected_sequence != u64::MAX
            && self.local_mac != 0
            && self.local_mac >> 48 == 0
            && self.local_ipv4 != 0
            && self.peer_ipv4 != 0
            && self.local_ipv4 != self.peer_ipv4
            && self.packet_count == 0
            && self.reserved == 0
    }
}

const _: () = assert!(core::mem::size_of::<NativeProtocolBootV2>() == 56);
const _: () = assert!(core::mem::align_of::<NativeProtocolBootV2>() == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeProtocolBootV3 {
    pub magic: u64,
    pub version: u16,
    pub size: u16,
    pub features: u32,
    pub packet_capability: u64,
    pub expected_sequence: u64,
    pub local_mac: u64,
    pub local_ipv4: u32,
    pub peer_ipv4: u32,
    pub packet_count: u32,
    pub reserved: u32,
}

impl NativeProtocolBootV3 {
    pub const fn ipv4_dispatch(v0: NativeProtocolBootV0) -> Self {
        Self {
            magic: v0.magic,
            version: NATIVE_PROTOCOL_BOOT_V3_VERSION,
            size: NATIVE_PROTOCOL_BOOT_V3_SIZE,
            features: NATIVE_PROTOCOL_FEATURE_PACKET_CONSUMER
                | NATIVE_PROTOCOL_FEATURE_ETHERNET_ARP
                | NATIVE_PROTOCOL_FEATURE_WAIT_LOOP
                | NATIVE_PROTOCOL_FEATURE_IPV4_DISPATCH,
            packet_capability: v0.packet_capability,
            expected_sequence: v0.expected_sequence,
            local_mac: v0.local_mac,
            local_ipv4: v0.local_ipv4,
            peer_ipv4: v0.peer_ipv4,
            packet_count: 2,
            reserved: 0,
        }
    }

    pub const fn is_valid_ipv4_dispatch(&self) -> bool {
        self.magic == NATIVE_PROTOCOL_BOOT_MAGIC
            && self.version == NATIVE_PROTOCOL_BOOT_V3_VERSION
            && self.size == NATIVE_PROTOCOL_BOOT_V3_SIZE
            && self.features
                == NATIVE_PROTOCOL_FEATURE_PACKET_CONSUMER
                    | NATIVE_PROTOCOL_FEATURE_ETHERNET_ARP
                    | NATIVE_PROTOCOL_FEATURE_WAIT_LOOP
                    | NATIVE_PROTOCOL_FEATURE_IPV4_DISPATCH
            && self.packet_capability != 0
            && self.expected_sequence != 0
            && self.expected_sequence < u64::MAX - 1
            && self.local_mac != 0
            && self.local_mac >> 48 == 0
            && self.local_ipv4 != 0
            && self.peer_ipv4 != 0
            && self.local_ipv4 != self.peer_ipv4
            && self.packet_count == 2
            && self.reserved == 0
    }
}

const _: () = assert!(core::mem::size_of::<NativeProtocolBootV3>() == 56);
const _: () = assert!(core::mem::align_of::<NativeProtocolBootV3>() == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeProtocolBootV4 {
    pub magic: u64,
    pub version: u16,
    pub size: u16,
    pub features: u32,
    pub packet_capability: u64,
    pub expected_sequence: u64,
    pub local_mac: u64,
    pub local_ipv4: u32,
    pub peer_ipv4: u32,
    pub packet_count: u32,
    pub reserved: u32,
}

impl NativeProtocolBootV4 {
    pub const fn icmp_echo(v0: NativeProtocolBootV0) -> Self {
        Self {
            magic: v0.magic,
            version: NATIVE_PROTOCOL_BOOT_V4_VERSION,
            size: NATIVE_PROTOCOL_BOOT_V4_SIZE,
            features: NATIVE_PROTOCOL_FEATURE_PACKET_CONSUMER
                | NATIVE_PROTOCOL_FEATURE_ETHERNET_ARP
                | NATIVE_PROTOCOL_FEATURE_WAIT_LOOP
                | NATIVE_PROTOCOL_FEATURE_IPV4_DISPATCH
                | NATIVE_PROTOCOL_FEATURE_ICMP_ECHO,
            packet_capability: v0.packet_capability,
            expected_sequence: v0.expected_sequence,
            local_mac: v0.local_mac,
            local_ipv4: v0.local_ipv4,
            peer_ipv4: v0.peer_ipv4,
            packet_count: 2,
            reserved: 0,
        }
    }

    pub const fn is_valid_icmp_echo(&self) -> bool {
        self.magic == NATIVE_PROTOCOL_BOOT_MAGIC
            && self.version == NATIVE_PROTOCOL_BOOT_V4_VERSION
            && self.size == NATIVE_PROTOCOL_BOOT_V4_SIZE
            && self.features
                == NATIVE_PROTOCOL_FEATURE_PACKET_CONSUMER
                    | NATIVE_PROTOCOL_FEATURE_ETHERNET_ARP
                    | NATIVE_PROTOCOL_FEATURE_WAIT_LOOP
                    | NATIVE_PROTOCOL_FEATURE_IPV4_DISPATCH
                    | NATIVE_PROTOCOL_FEATURE_ICMP_ECHO
            && self.packet_capability != 0
            && self.expected_sequence != 0
            && self.expected_sequence < u64::MAX - 1
            && self.local_mac != 0
            && self.local_mac >> 48 == 0
            && self.local_ipv4 != 0
            && self.peer_ipv4 != 0
            && self.local_ipv4 != self.peer_ipv4
            && self.packet_count == 2
            && self.reserved == 0
    }
}

const _: () = assert!(core::mem::size_of::<NativeProtocolBootV4>() == 56);
const _: () = assert!(core::mem::align_of::<NativeProtocolBootV4>() == 8);

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeProtocolBootV5 {
    pub magic: u64,
    pub version: u16,
    pub size: u16,
    pub features: u32,
    pub packet_capability: u64,
    pub expected_sequence: u64,
    pub local_mac: u64,
    pub local_ipv4: u32,
    pub peer_ipv4: u32,
    pub packet_count: u32,
    pub reserved: u32,
    pub egress_capability: u64,
    pub expected_egress_sequence: u64,
}

impl NativeProtocolBootV5 {
    pub const fn icmp_echo_reply(
        v0: NativeProtocolBootV0,
        egress_capability: u64,
        expected_egress_sequence: u64,
    ) -> Self {
        Self {
            magic: v0.magic,
            version: NATIVE_PROTOCOL_BOOT_V5_VERSION,
            size: NATIVE_PROTOCOL_BOOT_V5_SIZE,
            features: NATIVE_PROTOCOL_FEATURE_PACKET_CONSUMER
                | NATIVE_PROTOCOL_FEATURE_ETHERNET_ARP
                | NATIVE_PROTOCOL_FEATURE_WAIT_LOOP
                | NATIVE_PROTOCOL_FEATURE_IPV4_DISPATCH
                | NATIVE_PROTOCOL_FEATURE_ICMP_ECHO
                | NATIVE_PROTOCOL_FEATURE_PACKET_EGRESS,
            packet_capability: v0.packet_capability,
            expected_sequence: v0.expected_sequence,
            local_mac: v0.local_mac,
            local_ipv4: v0.local_ipv4,
            peer_ipv4: v0.peer_ipv4,
            packet_count: 2,
            reserved: 0,
            egress_capability,
            expected_egress_sequence,
        }
    }

    pub const fn is_valid_icmp_echo_reply(&self) -> bool {
        self.magic == NATIVE_PROTOCOL_BOOT_MAGIC
            && self.version == NATIVE_PROTOCOL_BOOT_V5_VERSION
            && self.size == NATIVE_PROTOCOL_BOOT_V5_SIZE
            && self.features
                == NATIVE_PROTOCOL_FEATURE_PACKET_CONSUMER
                    | NATIVE_PROTOCOL_FEATURE_ETHERNET_ARP
                    | NATIVE_PROTOCOL_FEATURE_WAIT_LOOP
                    | NATIVE_PROTOCOL_FEATURE_IPV4_DISPATCH
                    | NATIVE_PROTOCOL_FEATURE_ICMP_ECHO
                    | NATIVE_PROTOCOL_FEATURE_PACKET_EGRESS
            && self.packet_capability != 0
            && self.egress_capability != 0
            && self.packet_capability != self.egress_capability
            && self.expected_sequence != 0
            && self.expected_sequence < u64::MAX - 1
            && self.expected_egress_sequence != 0
            && self.expected_egress_sequence != u64::MAX
            && self.local_mac != 0
            && self.local_mac >> 48 == 0
            && self.local_ipv4 != 0
            && self.peer_ipv4 != 0
            && self.local_ipv4 != self.peer_ipv4
            && self.packet_count == 2
            && self.reserved == 0
    }
}

const _: () = assert!(core::mem::size_of::<NativeProtocolBootV5>() == 72);
const _: () = assert!(core::mem::align_of::<NativeProtocolBootV5>() == 8);

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
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_PACKET_EGRESS UINT32_C(512)"));
        assert!(
            network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS UINT32_C(1024)")
        );
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_EGRESS_FAULT UINT32_C(2048)"));
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_EGRESS_WAIT UINT32_C(4096)"));
        assert!(network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_EGRESS_REUSE UINT32_C(8192)"));
        assert!(
            network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_EGRESS_BATCH UINT32_C(16384)")
        );
        assert!(
            network_header.contains("GINUX_NATIVE_NETWORK_FEATURE_EGRESS_RESIDENT UINT32_C(32768)")
        );
        assert!(network_header.contains("GINUX_NATIVE_PACKET_QUEUE_CAPACITY UINT32_C(4)"));
        assert!(network_header.contains("GINUX_NATIVE_PACKET_WAIT_OPERATION UINT64_C(23)"));
        assert!(network_header.contains("GINUX_NATIVE_PACKET_WAIT_TIMEOUT_OPERATION UINT64_C(24)"));
        assert!(network_header.contains("GINUX_NATIVE_PACKET_SPACE_WAIT_OPERATION UINT64_C(25)"));
        assert!(network_header.contains("GINUX_NATIVE_PACKET_EGRESS_WAIT_OPERATION UINT64_C(28)"));
        assert!(
            network_header.contains("GINUX_NATIVE_PACKET_EGRESS_SPACE_WAIT_OPERATION UINT64_C(29)")
        );
        assert!(
            network_header.contains("GINUX_NATIVE_PACKET_EGRESS_DRAIN_WAIT_OPERATION UINT64_C(30)")
        );
        assert!(network_header
            .contains("GINUX_NATIVE_PACKET_WAIT_TIMED_OUT (UINT64_MAX - UINT64_C(5))"));
        assert!(network_header.contains("GINUX_NATIVE_PACKET_DRAINED (UINT64_MAX - UINT64_C(6))"));
        assert!(network_header.contains("GINUX_NATIVE_PACKET_WAIT_MAX_TIMEOUT_MS UINT64_C(1000)"));
        assert!(network_header.contains("sizeof(struct ginux_native_network_boot_v1) == 88"));
        assert!(network_header.contains("sizeof(struct ginux_native_network_boot_v2) == 96"));
        assert!(network_header.contains("sizeof(struct ginux_native_network_boot_v3) == 96"));
        assert!(network_header.contains("sizeof(struct ginux_native_network_boot_v4) == 96"));
        assert!(network_header.contains("sizeof(struct ginux_native_packet_request_v0) == 32"));
        assert!(network_header.contains("sizeof(struct ginux_native_network_boot_v0) == 80"));
        let protocol_header = include_str!("../../../specs/abi/native-protocol-bootstrap-v0.h");
        assert!(protocol_header.contains("UINT64_C(0x30425058554e4947)"));
        assert!(protocol_header.contains("GINUX_NATIVE_PROTOCOL_BOOT_SIZE UINT16_C(56)"));
        assert!(protocol_header.contains("sizeof(struct ginux_native_protocol_boot_v0) == 56"));
        assert!(protocol_header.contains("sizeof(struct ginux_native_protocol_boot_v1) == 56"));
        assert!(protocol_header.contains("sizeof(struct ginux_native_protocol_boot_v2) == 56"));
        assert!(protocol_header.contains("sizeof(struct ginux_native_protocol_boot_v3) == 56"));
        assert!(protocol_header.contains("sizeof(struct ginux_native_protocol_boot_v4) == 56"));
        assert!(protocol_header.contains("sizeof(struct ginux_native_protocol_boot_v5) == 72"));
    }

    #[test]
    fn protocol_bootstrap_layout_and_validation_are_frozen() {
        assert_eq!(NATIVE_PROTOCOL_BOOT_MAGIC.to_le_bytes(), *b"GINUXPB0");
        assert_eq!(core::mem::size_of::<NativeProtocolBootV0>(), 56);
        assert_eq!(core::mem::align_of::<NativeProtocolBootV0>(), 8);
        assert_eq!(core::mem::offset_of!(NativeProtocolBootV0, magic), 0);
        assert_eq!(core::mem::offset_of!(NativeProtocolBootV0, version), 8);
        assert_eq!(core::mem::offset_of!(NativeProtocolBootV0, size), 10);
        assert_eq!(core::mem::offset_of!(NativeProtocolBootV0, features), 12);
        assert_eq!(
            core::mem::offset_of!(NativeProtocolBootV0, packet_capability),
            16
        );
        assert_eq!(
            core::mem::offset_of!(NativeProtocolBootV0, expected_sequence),
            24
        );
        assert_eq!(core::mem::offset_of!(NativeProtocolBootV0, local_mac), 32);
        assert_eq!(core::mem::offset_of!(NativeProtocolBootV0, local_ipv4), 40);
        assert_eq!(core::mem::offset_of!(NativeProtocolBootV0, peer_ipv4), 44);
        assert_eq!(core::mem::offset_of!(NativeProtocolBootV0, reserved), 48);
        let valid = NativeProtocolBootV0::arp_consumer(
            1,
            2,
            0x5254_0012_3456,
            u32::from_be_bytes([10, 0, 2, 15]),
            u32::from_be_bytes([10, 0, 2, 2]),
        );
        assert!(valid.is_valid_arp_consumer());
        for invalid in [
            NativeProtocolBootV0 { magic: 0, ..valid },
            NativeProtocolBootV0 {
                expected_sequence: 0,
                ..valid
            },
            NativeProtocolBootV0 {
                local_ipv4: valid.peer_ipv4,
                ..valid
            },
            NativeProtocolBootV0 {
                reserved: 1,
                ..valid
            },
        ] {
            assert!(!invalid.is_valid_arp_consumer());
        }
        let wait = NativeProtocolBootV1::wait_loop(valid, 2);
        assert!(wait.is_valid_wait_loop());
        assert_eq!(core::mem::size_of::<NativeProtocolBootV1>(), 56);
        assert_eq!(
            core::mem::offset_of!(NativeProtocolBootV1, packet_count),
            48
        );
        assert_eq!(core::mem::offset_of!(NativeProtocolBootV1, reserved), 52);
        assert!(!NativeProtocolBootV1 {
            packet_count: 1,
            ..wait
        }
        .is_valid_wait_loop());
        let supervised = NativeProtocolBootV2::supervised_loop(valid);
        assert!(supervised.is_valid_supervised_loop());
        assert_eq!(core::mem::size_of::<NativeProtocolBootV2>(), 56);
        assert!(!NativeProtocolBootV2 {
            packet_count: 2,
            ..supervised
        }
        .is_valid_supervised_loop());
        let ipv4 = NativeProtocolBootV3::ipv4_dispatch(valid);
        assert!(ipv4.is_valid_ipv4_dispatch());
        assert!(!NativeProtocolBootV3 {
            packet_count: 1,
            ..ipv4
        }
        .is_valid_ipv4_dispatch());
        let icmp = NativeProtocolBootV4::icmp_echo(valid);
        assert!(icmp.is_valid_icmp_echo());
        assert!(!NativeProtocolBootV4 {
            packet_count: 1,
            ..icmp
        }
        .is_valid_icmp_echo());
        let reply = NativeProtocolBootV5::icmp_echo_reply(valid, 3, 1);
        assert!(reply.is_valid_icmp_echo_reply());
        assert_eq!(core::mem::size_of::<NativeProtocolBootV5>(), 72);
        assert_eq!(
            core::mem::offset_of!(NativeProtocolBootV5, egress_capability),
            56
        );
        assert_eq!(
            core::mem::offset_of!(NativeProtocolBootV5, expected_egress_sequence),
            64
        );
        assert!(!NativeProtocolBootV5 {
            egress_capability: reply.packet_capability,
            ..reply
        }
        .is_valid_icmp_echo_reply());
        assert!(!NativeProtocolBootV5 {
            expected_egress_sequence: 0,
            ..reply
        }
        .is_valid_icmp_echo_reply());
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
        let egress = NativeNetworkBootV2::egress_transmit(interrupted, 7, 1);
        assert!(egress.is_valid_egress_transmit());
        assert_eq!(egress.v0_prefix(), interrupted);
        assert_eq!(core::mem::size_of::<NativeNetworkBootV2>(), 96);
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV2, egress_capability),
            72
        );
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV2, expected_egress_sequence),
            80
        );
        assert_eq!(core::mem::offset_of!(NativeNetworkBootV2, reserved), 88);
        for invalid in [
            NativeNetworkBootV2 {
                egress_capability: egress.device_capability,
                ..egress
            },
            NativeNetworkBootV2 {
                expected_egress_sequence: 0,
                ..egress
            },
            NativeNetworkBootV2 {
                features: egress.features | NATIVE_NETWORK_FEATURE_ARP_PROBE,
                ..egress
            },
            NativeNetworkBootV2 {
                reserved: 1,
                ..egress
            },
        ] {
            assert!(!invalid.is_valid_egress_transmit());
        }
        let reliable = NativeNetworkBootV3::reliable_egress_transmit(interrupted, 7, 1, false);
        assert!(reliable.is_valid_reliable_egress_transmit());
        assert!(!reliable.requests_fault_after_claim());
        assert_eq!(reliable.v0_prefix(), interrupted);
        let faulting_reliable =
            NativeNetworkBootV3::reliable_egress_transmit(interrupted, 7, 1, true);
        assert!(faulting_reliable.is_valid_reliable_egress_transmit());
        assert!(faulting_reliable.requests_fault_after_claim());
        assert_eq!(core::mem::size_of::<NativeNetworkBootV3>(), 96);
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV3, egress_capability),
            72
        );
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV3, expected_egress_sequence),
            80
        );
        assert_eq!(core::mem::offset_of!(NativeNetworkBootV3, reserved), 88);
        for invalid in [
            NativeNetworkBootV3 {
                version: NATIVE_NETWORK_BOOT_V2_VERSION,
                ..reliable
            },
            NativeNetworkBootV3 {
                features: reliable.features & !NATIVE_NETWORK_FEATURE_RELIABLE_EGRESS,
                ..reliable
            },
            NativeNetworkBootV3 {
                features: reliable.features | NATIVE_NETWORK_FEATURE_PACKET_BATCH,
                ..reliable
            },
            NativeNetworkBootV3 {
                expected_egress_sequence: 0,
                ..reliable
            },
            NativeNetworkBootV3 {
                reserved: 1,
                ..reliable
            },
        ] {
            assert!(!invalid.is_valid_reliable_egress_transmit());
        }
        let waiting = NativeNetworkBootV4::waiting_reliable_egress_transmit(interrupted, 7, 1);
        assert!(waiting.is_valid_waiting_reliable_egress_transmit());
        assert_eq!(waiting.v0_prefix(), interrupted);
        assert_eq!(core::mem::size_of::<NativeNetworkBootV4>(), 96);
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV4, egress_capability),
            72
        );
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV4, expected_egress_sequence),
            80
        );
        assert_eq!(core::mem::offset_of!(NativeNetworkBootV4, reserved), 88);
        for invalid in [
            NativeNetworkBootV4 {
                version: NATIVE_NETWORK_BOOT_V3_VERSION,
                ..waiting
            },
            NativeNetworkBootV4 {
                features: waiting.features & !NATIVE_NETWORK_FEATURE_EGRESS_WAIT,
                ..waiting
            },
            NativeNetworkBootV4 {
                features: waiting.features | NATIVE_NETWORK_FEATURE_EGRESS_FAULT,
                ..waiting
            },
            NativeNetworkBootV4 {
                expected_egress_sequence: 0,
                ..waiting
            },
            NativeNetworkBootV4 {
                reserved: 1,
                ..waiting
            },
        ] {
            assert!(!invalid.is_valid_waiting_reliable_egress_transmit());
        }
        let resident = NativeNetworkBootV5::resident_reliable_egress_transmit(interrupted, 7, 1);
        assert!(resident.is_valid_resident_reliable_egress_transmit());
        assert_eq!(resident.v0_prefix(), interrupted);
        assert_eq!(core::mem::size_of::<NativeNetworkBootV5>(), 96);
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV5, egress_capability),
            72
        );
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV5, expected_egress_sequence),
            80
        );
        assert_eq!(core::mem::offset_of!(NativeNetworkBootV5, reserved), 88);
        for invalid in [
            NativeNetworkBootV5 {
                version: NATIVE_NETWORK_BOOT_V4_VERSION,
                ..resident
            },
            NativeNetworkBootV5 {
                features: resident.features & !NATIVE_NETWORK_FEATURE_EGRESS_REUSE,
                ..resident
            },
            NativeNetworkBootV5 {
                features: resident.features | NATIVE_NETWORK_FEATURE_EGRESS_FAULT,
                ..resident
            },
            NativeNetworkBootV5 {
                expected_egress_sequence: u64::MAX,
                ..resident
            },
            NativeNetworkBootV5 {
                reserved: 1,
                ..resident
            },
        ] {
            assert!(!invalid.is_valid_resident_reliable_egress_transmit());
        }
        let batch = NativeNetworkBootV6::resident_reliable_egress_batch(interrupted, 7, 1);
        assert!(batch.is_valid_resident_reliable_egress_batch());
        assert_eq!(batch.v0_prefix(), interrupted);
        assert_eq!(core::mem::size_of::<NativeNetworkBootV6>(), 96);
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV6, egress_capability),
            72
        );
        assert_eq!(
            core::mem::offset_of!(NativeNetworkBootV6, expected_egress_sequence),
            80
        );
        assert_eq!(core::mem::offset_of!(NativeNetworkBootV6, reserved), 88);
        for invalid in [
            NativeNetworkBootV6 {
                version: NATIVE_NETWORK_BOOT_V5_VERSION,
                ..batch
            },
            NativeNetworkBootV6 {
                features: batch.features & !NATIVE_NETWORK_FEATURE_EGRESS_BATCH,
                ..batch
            },
            NativeNetworkBootV6 {
                features: batch.features | NATIVE_NETWORK_FEATURE_EGRESS_FAULT,
                ..batch
            },
            NativeNetworkBootV6 {
                expected_egress_sequence: u64::MAX - 2,
                ..batch
            },
            NativeNetworkBootV6 {
                reserved: 1,
                ..batch
            },
        ] {
            assert!(!invalid.is_valid_resident_reliable_egress_batch());
        }
        let resident_loop = NativeNetworkBootV7::resident_reliable_egress_loop(interrupted, 7, 1);
        assert!(resident_loop.is_valid_resident_reliable_egress_loop());
        assert_eq!(resident_loop.v0_prefix(), interrupted);
        assert_eq!(core::mem::size_of::<NativeNetworkBootV7>(), 96);
        for invalid in [
            NativeNetworkBootV7 {
                version: NATIVE_NETWORK_BOOT_V6_VERSION,
                ..resident_loop
            },
            NativeNetworkBootV7 {
                features: resident_loop.features & !NATIVE_NETWORK_FEATURE_EGRESS_RESIDENT,
                ..resident_loop
            },
            NativeNetworkBootV7 {
                expected_egress_sequence: u64::MAX - 3,
                ..resident_loop
            },
            NativeNetworkBootV7 {
                reserved: 1,
                ..resident_loop
            },
        ] {
            assert!(!invalid.is_valid_resident_reliable_egress_loop());
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

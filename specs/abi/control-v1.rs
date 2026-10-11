// Machine-readable constants shared by the kernel and bootstrap proof builder.
pub const VERSION: u16 = 1;
pub const FEATURES: u16 = 0xffff;
pub const MAX_BUFFERED_CONSOLE_BYTES: u32 = 64;
pub const DESCRIPTOR_SIZE: usize = 16;
pub const FAULT_DESCRIPTOR_SIZE: usize = 48;
pub const LIFECYCLE_DESCRIPTOR_SIZE: usize = 24;
pub const IPC_DELIVERY_DESCRIPTOR_SIZE: usize = 32;
pub const LIFECYCLE_IDENTITY_DESCRIPTOR_SIZE: usize = 32;
pub const MANAGEMENT_REQUEST_SIZE: usize = 32;
pub const MANAGEMENT_CREATE_AUXILIARY: u16 = 1;
pub const MANAGEMENT_STOP_AUXILIARY: u16 = 2;
pub const LIFECYCLE_SERVICE_FAULT: u8 = 1;
pub const LIFECYCLE_OVERFLOW: u8 = 2;
pub const LIFECYCLE_AUXILIARY_STOPPED: u8 = 3;
pub const LIFECYCLE_AUXILIARY_FAULT: u8 = 4;

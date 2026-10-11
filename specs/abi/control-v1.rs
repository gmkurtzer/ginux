// Machine-readable constants shared by the kernel and bootstrap proof builder.
pub const VERSION: u16 = 1;
pub const FEATURES: u16 = 0x3fff;
pub const MAX_BUFFERED_CONSOLE_BYTES: u32 = 64;
pub const DESCRIPTOR_SIZE: usize = 16;
pub const FAULT_DESCRIPTOR_SIZE: usize = 48;
pub const LIFECYCLE_DESCRIPTOR_SIZE: usize = 24;
pub const IPC_DELIVERY_DESCRIPTOR_SIZE: usize = 32;

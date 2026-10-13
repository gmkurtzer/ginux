#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use ginux_native_abi::{
    native_control_abi_is_compatible, NativeControlDescriptorV4, NativeHostInspectRecordV0,
    NativeHostInspectRequestV0, NativeRootDirectoryRequestV4, NATIVE_CALL_FAILED,
    NATIVE_CONSOLE_READ_LINE_OPERATION, NATIVE_CONSOLE_WRITE_OPERATION,
    NATIVE_CONTAINER_CONTROL_OPERATION, NATIVE_CONTAINER_HOST_SHUTDOWN,
    NATIVE_CONTAINER_INJECT_FAILURE, NATIVE_CONTAINER_QUERY, NATIVE_CONTAINER_RESTART,
    NATIVE_CONTAINER_STATE_FAILED, NATIVE_CONTAINER_STATE_RUNNING, NATIVE_CONTAINER_STATE_STOPPED,
    NATIVE_CONTAINER_STOP, NATIVE_CONTROL_DESCRIPTOR_OPERATION, NATIVE_CONTROL_DESCRIPTOR_SIZE,
    NATIVE_CONTROL_DISCOVERY_OPERATION, NATIVE_HOST_INSPECT_OPERATION,
    NATIVE_HOST_INSPECT_RECORD_SIZE, NATIVE_HOST_INSPECT_REQUEST_SIZE,
    NATIVE_HOST_INSPECT_STATE_READY, NATIVE_HOST_INSPECT_STATE_STOPPING,
    NATIVE_HOST_INSPECT_VERSION, NATIVE_HOST_PROCESS_KIND_CONTAINER_INIT,
    NATIVE_HOST_PROCESS_KIND_CONTROLLER, NATIVE_HOST_PROC_ENUMERATE, NATIVE_HOST_PROC_STATUS,
    NATIVE_ROOT_DIRECTORY_RECORD_HEADER_SIZE, NATIVE_ROOT_DIRECTORY_REQUEST_SIZE,
    NATIVE_ROOT_MAX_DIRECTORY_RECORD_SIZE, NATIVE_ROOT_OPERATION, NATIVE_ROOT_READ_DIRECTORY,
    NATIVE_SERVICE_EXIT_COMPLETED, NATIVE_SERVICE_EXIT_OPERATION, NATIVE_SERVICE_EXIT_REJECTED,
};

const MAX_LINE: usize = 32;
static CONTROLLER_IDENTITY: &[u8] = b"GINUX_CONTROLLER_V0";

global_asm!(
    r#"
    .text
    .global _start
_start:
    mov rdi, rbx
    mov rsi, rcx
    mov rdx, r14
    call ginux_controller_main
    ud2
"#
);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    exit(NATIVE_SERVICE_EXIT_REJECTED)
}

#[no_mangle]
extern "C" fn ginux_controller_main(console: u64, host: u64, root: u64) -> ! {
    let identity = unsafe { core::ptr::read_volatile(&CONTROLLER_IDENTITY) };
    if identity != b"GINUX_CONTROLLER_V0"
        || console == 0
        || host == 0
        || root == 0
        || !native_control_abi_is_compatible(call(NATIVE_CONTROL_DISCOVERY_OPERATION, 0, 0, 0))
    {
        exit(NATIVE_SERVICE_EXIT_REJECTED);
    }
    let mut descriptor = NativeControlDescriptorV4::zeroed();
    if call(
        NATIVE_CONTROL_DESCRIPTOR_OPERATION,
        0,
        (&mut descriptor as *mut NativeControlDescriptorV4) as u64,
        u64::from(NATIVE_CONTROL_DESCRIPTOR_SIZE),
    ) != u64::from(NATIVE_CONTROL_DESCRIPTOR_SIZE)
        || !descriptor.is_compatible()
    {
        exit(NATIVE_SERVICE_EXIT_REJECTED);
    }
    let mut line = [0_u8; MAX_LINE];
    if call(
        NATIVE_CONSOLE_READ_LINE_OPERATION,
        0,
        line.as_mut_ptr() as u64,
        line.len() as u64,
    ) != NATIVE_CALL_FAILED
    {
        exit(NATIVE_SERVICE_EXIT_REJECTED);
    }

    write(console, b"\r\nGinux controller v0 (ring 3)\r\n");
    write(console, b"Type 'help' for commands.\r\n");
    loop {
        write(console, b"ginux> ");
        let length = call(
            NATIVE_CONSOLE_READ_LINE_OPERATION,
            console,
            line.as_mut_ptr() as u64,
            line.len() as u64,
        );
        let Ok(length) = usize::try_from(length) else {
            exit(NATIVE_SERVICE_EXIT_REJECTED);
        };
        let Some(command) = line.get(..length) else {
            exit(NATIVE_SERVICE_EXIT_REJECTED);
        };
        match command {
            b"help" => {
                write(console, b"help        Show this command list\r\n");
                write(console, b"status      Show controller and host state\r\n");
                write(console, b"components  Show embedded image components\r\n");
                write(console, b"ls [PATH]   List the embedded measured root\r\n");
                write(console, b"ps          Show live host processes\r\n");
                write(console, b"container status default\r\n");
                write(console, b"container stop default\r\n");
                write(console, b"container start default\r\n");
                write(console, b"container restart default\r\n");
                write(console, b"container fail default\r\n");
                write(console, b"shutdown    Shut down Ginux cleanly\r\n");
            }
            b"status" => {
                write(console, b"controller: running (ring 3)\r\n");
                write(console, b"containers: 1 configured (default)\r\n");
                write(console, b"default: ");
                write_container_state(console, query_container(host));
                write(console, b"integrity: sealed\r\n");
            }
            b"components" => {
                write(console, b"GINUX.EFI component catalog v0:\r\n");
                write(console, b"- kernel: active, ring 0\r\n");
                write(console, b"- controller: active, ring 3\r\n");
                write(console, b"- boot modules: embedded, sealed\r\n");
            }
            b"ps" => show_processes(console, host),
            b"container status default" => {
                write(console, b"default: ");
                write_container_state(console, query_container(host));
            }
            b"container stop default" => {
                if call(
                    NATIVE_CONTAINER_CONTROL_OPERATION,
                    host,
                    NATIVE_CONTAINER_STOP,
                    0,
                ) == NATIVE_CONTAINER_STATE_STOPPED
                {
                    write(console, b"default: stopped\r\n");
                    exit(NATIVE_SERVICE_EXIT_COMPLETED);
                } else {
                    write(console, b"container: stop rejected\r\n");
                }
            }
            b"container start default" | b"container restart default" => {
                if call(
                    NATIVE_CONTAINER_CONTROL_OPERATION,
                    host,
                    NATIVE_CONTAINER_RESTART,
                    0,
                ) == NATIVE_CONTAINER_STATE_STOPPED
                {
                    write(console, b"default: restarting\r\n");
                    exit(NATIVE_SERVICE_EXIT_COMPLETED);
                }
                write(console, b"container: restart rejected\r\n");
            }
            b"container fail default" => {
                if call(
                    NATIVE_CONTAINER_CONTROL_OPERATION,
                    host,
                    NATIVE_CONTAINER_INJECT_FAILURE,
                    0,
                ) == NATIVE_CONTAINER_STATE_FAILED
                {
                    write(console, b"default: failed (contained)\r\n");
                    exit(NATIVE_SERVICE_EXIT_COMPLETED);
                } else {
                    write(console, b"container: failure injection rejected\r\n");
                }
            }
            b"shutdown" => {
                write(console, b"Shutting down Ginux.\r\n");
                if call(
                    NATIVE_CONTAINER_CONTROL_OPERATION,
                    host,
                    NATIVE_CONTAINER_HOST_SHUTDOWN,
                    0,
                ) == NATIVE_CALL_FAILED
                {
                    exit(NATIVE_SERVICE_EXIT_REJECTED);
                }
                exit(NATIVE_SERVICE_EXIT_COMPLETED);
            }
            b"" => {}
            command if command == b"ls" || command.starts_with(b"ls ") => {
                let supplied = command.get(2..).unwrap_or_default();
                let supplied = supplied.strip_prefix(b" ").unwrap_or(supplied);
                let path = supplied.strip_prefix(b"/").unwrap_or(supplied);
                if path == b"proc" {
                    list_proc_directory(console, host, b"");
                } else if let Some(path) = path.strip_prefix(b"proc/") {
                    list_proc_directory(console, host, path);
                } else {
                    list_directory(console, root, path);
                }
            }
            _ => write(console, b"Unknown command. Type 'help'.\r\n"),
        }
    }
}

fn query_container(host: u64) -> u64 {
    call(
        NATIVE_CONTAINER_CONTROL_OPERATION,
        host,
        NATIVE_CONTAINER_QUERY,
        0,
    )
}

fn write_container_state(console: u64, state: u64) {
    write(
        console,
        match state {
            NATIVE_CONTAINER_STATE_RUNNING => b"running\r\n",
            NATIVE_CONTAINER_STATE_STOPPED => b"stopped\r\n",
            NATIVE_CONTAINER_STATE_FAILED => b"failed\r\n",
            _ => b"unknown\r\n",
        },
    );
}

fn write(console: u64, bytes: &[u8]) {
    if bytes.is_empty() {
        exit(NATIVE_SERVICE_EXIT_REJECTED);
    }
    for chunk in bytes.chunks(64) {
        if call(
            NATIVE_CONSOLE_WRITE_OPERATION,
            console,
            chunk.as_ptr() as u64,
            chunk.len() as u64,
        ) != chunk.len() as u64
        {
            exit(NATIVE_SERVICE_EXIT_REJECTED);
        }
    }
}

fn list_directory(console: u64, root: u64, path: &[u8]) {
    let (path_address, path_size) = if path.is_empty() {
        (0, 0)
    } else {
        (path.as_ptr() as u64, path.len() as u64)
    };
    let mut cursor = 0;
    let mut output = [0_u8; NATIVE_ROOT_MAX_DIRECTORY_RECORD_SIZE];
    loop {
        let request = NativeRootDirectoryRequestV4 {
            size: NATIVE_ROOT_DIRECTORY_REQUEST_SIZE,
            version: 4,
            reserved: 0,
            path_address,
            path_size,
            cursor,
            output_address: output.as_mut_ptr() as u64,
            output_capacity: output.len() as u64,
        };
        let result = call(
            NATIVE_ROOT_OPERATION,
            root,
            NATIVE_ROOT_READ_DIRECTORY,
            (&request as *const NativeRootDirectoryRequestV4) as u64,
        );
        if result == 0 {
            return;
        }
        let Ok(record_size) = usize::try_from(result) else {
            write(console, b"ls: inspection failed\r\n");
            return;
        };
        if record_size > output.len()
            || record_size < NATIVE_ROOT_DIRECTORY_RECORD_HEADER_SIZE
            || read_u32(&output, 0) as usize != record_size
            || read_u16(&output, 4) != 4
        {
            write(console, b"ls: path not found or not a directory\r\n");
            return;
        }
        let next_cursor = read_u64(&output, 8);
        let mode = read_u32(&output, 16);
        let name_size = read_u32(&output, 20) as usize;
        let Some(name_end) = NATIVE_ROOT_DIRECTORY_RECORD_HEADER_SIZE.checked_add(name_size) else {
            write(console, b"ls: invalid root record\r\n");
            return;
        };
        let Some(name) = output.get(NATIVE_ROOT_DIRECTORY_RECORD_HEADER_SIZE..name_end) else {
            write(console, b"ls: invalid root record\r\n");
            return;
        };
        let mut line = [0_u8; 258];
        line[..name.len()].copy_from_slice(name);
        let mut line_size = name.len();
        if mode & 0o170000 == 0o040000 {
            line[line_size] = b'/';
            line_size += 1;
        }
        line[line_size..line_size + 2].copy_from_slice(b"\r\n");
        line_size += 2;
        write(console, &line[..line_size]);
        if next_cursor <= cursor {
            write(console, b"ls: invalid root cursor\r\n");
            return;
        }
        cursor = next_cursor;
    }
}

fn show_processes(console: u64, host: u64) {
    write(console, b"PID PPID STATE NAME\r\n");
    let mut cursor = 0;
    loop {
        let Some(directory_record) = proc_request(host, NATIVE_HOST_PROC_ENUMERATE, cursor) else {
            return;
        };
        if directory_record.next_cursor <= cursor {
            write(console, b"ps: inspection failed\r\n");
            return;
        }
        let Some(record) = proc_request(host, NATIVE_HOST_PROC_STATUS, directory_record.pid) else {
            write(console, b"ps: inspection failed\r\n");
            return;
        };
        write_decimal(console, record.pid);
        write(console, b" ");
        write_decimal(console, record.parent_pid);
        write(console, b" ");
        write(
            console,
            match record.state {
                NATIVE_HOST_INSPECT_STATE_READY => b"R",
                NATIVE_HOST_INSPECT_STATE_STOPPING => b"S",
                _ => b"unknown",
            },
        );
        write(console, b" ");
        write(
            console,
            match record.kind {
                NATIVE_HOST_PROCESS_KIND_CONTROLLER => b"controller",
                NATIVE_HOST_PROCESS_KIND_CONTAINER_INIT => b"default:init",
                _ => b"unknown",
            },
        );
        write(console, b"\r\n");
        cursor = directory_record.next_cursor;
    }
}

fn list_proc_directory(console: u64, host: u64, path: &[u8]) {
    if path.is_empty() {
        let mut cursor = 0;
        loop {
            let Some(record) = next_process(host, cursor) else {
                return;
            };
            write_decimal(console, record.pid);
            write(console, b"/\r\n");
            cursor = record.next_cursor;
        }
    }
    let Some(pid) = parse_decimal(path) else {
        write(console, b"ls: path not found or not a directory\r\n");
        return;
    };
    let mut cursor = 0;
    loop {
        let Some(record) = next_process(host, cursor) else {
            write(console, b"ls: path not found or not a directory\r\n");
            return;
        };
        if record.pid == pid {
            write(console, b"status\r\n");
            return;
        }
        cursor = record.next_cursor;
    }
}

fn next_process(host: u64, cursor: u64) -> Option<NativeHostInspectRecordV0> {
    proc_request(host, NATIVE_HOST_PROC_ENUMERATE, cursor)
}

fn proc_request(host: u64, command: u16, selector: u64) -> Option<NativeHostInspectRecordV0> {
    let mut record = NativeHostInspectRecordV0::zeroed();
    let request = NativeHostInspectRequestV0 {
        size: NATIVE_HOST_INSPECT_REQUEST_SIZE,
        version: NATIVE_HOST_INSPECT_VERSION,
        command,
        cursor: selector,
        output_address: (&mut record as *mut NativeHostInspectRecordV0) as u64,
        output_size: u64::from(NATIVE_HOST_INSPECT_RECORD_SIZE),
    };
    let result = call(
        NATIVE_HOST_INSPECT_OPERATION,
        host,
        (&request as *const NativeHostInspectRequestV0) as u64,
        0,
    );
    (result == u64::from(NATIVE_HOST_INSPECT_RECORD_SIZE)
        && record.size == NATIVE_HOST_INSPECT_RECORD_SIZE
        && record.version == NATIVE_HOST_INSPECT_VERSION
        && record.reserved == 0)
        .then_some(record)
}

fn parse_decimal(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() {
        return None;
    }
    bytes.iter().try_fold(0_u64, |value, byte| {
        value.checked_mul(10)?.checked_add(u64::from(
            byte.checked_sub(b'0').filter(|digit| *digit <= 9)?,
        ))
    })
}

fn write_decimal(console: u64, mut value: u64) {
    let mut bytes = [0_u8; 20];
    let mut start = bytes.len();
    loop {
        start -= 1;
        bytes[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    write(console, &bytes[start..]);
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn call(number: u64, capability: u64, first: u64, second: u64) -> u64 {
    let result: u64;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") number => result,
            in("rdi") capability,
            in("rsi") first,
            in("rdx") second,
            options(nostack)
        );
    }
    result
}

fn exit(status: u8) -> ! {
    unsafe {
        asm!(
            "int 0x80",
            in("rax") NATIVE_SERVICE_EXIT_OPERATION,
            in("rsi") u64::from(status),
            options(noreturn)
        );
    }
}

const _: () = assert!(NATIVE_CALL_FAILED == u64::MAX - 2);

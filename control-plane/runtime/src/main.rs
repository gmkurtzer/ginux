#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use ginux_native_abi::{
    native_control_abi_is_compatible, NativeContainerRequestV0, NativeControlDescriptorV4,
    NativeHostInspectRecordV0, NativeHostInspectRequestV0, NativeRootDirectoryRequestV4,
    NATIVE_CALL_FAILED, NATIVE_CONSOLE_READ_LINE_OPERATION, NATIVE_CONSOLE_WRITE_OPERATION,
    NATIVE_CONTAINER_CONTROL_OPERATION, NATIVE_CONTAINER_CREATE, NATIVE_CONTAINER_HOST_SHUTDOWN,
    NATIVE_CONTAINER_INJECT_FAILURE, NATIVE_CONTAINER_MAX_NAME_SIZE, NATIVE_CONTAINER_QUERY,
    NATIVE_CONTAINER_RESTART, NATIVE_CONTAINER_SET_AUTOSTART, NATIVE_CONTAINER_SET_CPU,
    NATIVE_CONTAINER_SET_INIT, NATIVE_CONTAINER_SET_MEMORY_MIB, NATIVE_CONTAINER_STATE_CONFIGURED,
    NATIVE_CONTAINER_STATE_CREATED, NATIVE_CONTAINER_STATE_FAILED, NATIVE_CONTAINER_STATE_RUNNING,
    NATIVE_CONTAINER_STATE_STOPPED, NATIVE_CONTAINER_STOP, NATIVE_CONTROL_DESCRIPTOR_OPERATION,
    NATIVE_CONTROL_DESCRIPTOR_SIZE, NATIVE_CONTROL_DISCOVERY_OPERATION,
    NATIVE_HOST_INSPECT_OPERATION, NATIVE_HOST_INSPECT_RECORD_SIZE,
    NATIVE_HOST_INSPECT_REQUEST_SIZE, NATIVE_HOST_INSPECT_STATE_READY,
    NATIVE_HOST_INSPECT_STATE_STOPPING, NATIVE_HOST_INSPECT_VERSION,
    NATIVE_HOST_PROCESS_KIND_CONTAINER_INIT, NATIVE_HOST_PROCESS_KIND_CONTROLLER,
    NATIVE_HOST_PROC_ENUMERATE, NATIVE_HOST_PROC_STATUS, NATIVE_ROOT_DIRECTORY_RECORD_HEADER_SIZE,
    NATIVE_ROOT_DIRECTORY_REQUEST_SIZE, NATIVE_ROOT_MAX_DIRECTORY_RECORD_SIZE,
    NATIVE_ROOT_OPERATION, NATIVE_ROOT_READ_DIRECTORY, NATIVE_SERVICE_EXIT_COMPLETED,
    NATIVE_SERVICE_EXIT_OPERATION, NATIVE_SERVICE_EXIT_REJECTED,
};

const MAX_LINE: usize = 64;
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
                write(console, b"container new NAME\r\n");
                write(console, b"container set NAME init /sbin/init\r\n");
                write(console, b"container set NAME cpu 1\r\n");
                write(console, b"container set NAME memory-mib 128\r\n");
                write(console, b"container set NAME autostart true\r\n");
                write(
                    console,
                    b"container status|start|stop|restart|fail NAME\r\n",
                );
                write(console, b"shutdown    Shut down Ginux cleanly\r\n");
            }
            b"status" => {
                write(console, b"controller: running (ring 3)\r\n");
                write_container_summary(console, host);
                write(console, b"integrity: sealed\r\n");
            }
            b"components" => {
                write(console, b"GINUX.EFI component catalog v0:\r\n");
                write(console, b"- kernel: active, ring 0\r\n");
                write(console, b"- controller: active, ring 3\r\n");
                write(console, b"- boot modules: embedded, sealed\r\n");
            }
            b"ps" => show_processes(console, host),
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
            command if command.starts_with(b"container ") => {
                handle_container_command(console, host, command)
            }
            _ => write(console, b"Unknown command. Type 'help'.\r\n"),
        }
    }
}

fn write_container_summary(console: u64, host: u64) {
    let mut request = NativeContainerRequestV0::zeroed();
    let state = container_call(host, NATIVE_CONTAINER_QUERY, &mut request);
    if state == 0 {
        write(console, b"containers: 0 configured\r\n");
        return;
    }
    write(console, b"containers: 1 runtime (");
    write(console, request_name(&request));
    write(console, b")\r\n");
    write(console, request_name(&request));
    write(console, b": ");
    write_container_state(console, state);
}

fn write_container_state(console: u64, state: u64) {
    write(
        console,
        match state {
            NATIVE_CONTAINER_STATE_RUNNING => b"running\r\n",
            NATIVE_CONTAINER_STATE_STOPPED => b"stopped\r\n",
            NATIVE_CONTAINER_STATE_FAILED => b"failed\r\n",
            NATIVE_CONTAINER_STATE_CREATED => b"created (incomplete)\r\n",
            NATIVE_CONTAINER_STATE_CONFIGURED => b"configured\r\n",
            _ => b"unknown\r\n",
        },
    );
}

fn handle_container_command(console: u64, host: u64, command: &[u8]) {
    let mut words = command.split(|byte| *byte == b' ');
    let verb = words.nth(1).unwrap_or_default();
    let name = words.next().unwrap_or_default();
    if !valid_container_name(name) {
        write(console, b"container: invalid name\r\n");
        return;
    }
    let mut request = request_for_name(name);
    match verb {
        b"new" if words.next().is_none() => {
            if container_call(host, NATIVE_CONTAINER_CREATE, &mut request)
                == NATIVE_CONTAINER_STATE_CREATED
            {
                write_name_state(console, name, b"created\r\n");
            } else {
                write(console, b"container: create rejected\r\n");
            }
        }
        b"set" => {
            let field = words.next().unwrap_or_default();
            let value = words.next().unwrap_or_default();
            if words.next().is_some() || !set_request_value(&mut request, field, value) {
                write(console, b"container: invalid setting\r\n");
                return;
            }
            let operation = match field {
                b"init" => NATIVE_CONTAINER_SET_INIT,
                b"cpu" => NATIVE_CONTAINER_SET_CPU,
                b"memory-mib" => NATIVE_CONTAINER_SET_MEMORY_MIB,
                b"autostart" => NATIVE_CONTAINER_SET_AUTOSTART,
                _ => {
                    write(console, b"container: invalid setting\r\n");
                    return;
                }
            };
            let state = container_call(host, operation, &mut request);
            if matches!(
                state,
                NATIVE_CONTAINER_STATE_CREATED | NATIVE_CONTAINER_STATE_CONFIGURED
            ) {
                write(console, name);
                write(console, b": setting accepted; ");
                write_container_state(console, state);
            } else {
                write(console, b"container: setting rejected\r\n");
            }
        }
        b"status" if words.next().is_none() => {
            let state = container_call(host, NATIVE_CONTAINER_QUERY, &mut request);
            if state == NATIVE_CALL_FAILED {
                write(console, b"container: not found\r\n");
            } else {
                write(console, name);
                write(console, b": ");
                write_container_state(console, state);
            }
        }
        b"start" | b"restart" if words.next().is_none() => {
            if container_call(host, NATIVE_CONTAINER_RESTART, &mut request)
                == NATIVE_CONTAINER_STATE_STOPPED
            {
                write_name_state(console, name, b"starting\r\n");
            } else {
                write(console, b"container: start rejected\r\n");
            }
        }
        b"stop" if words.next().is_none() => {
            if container_call(host, NATIVE_CONTAINER_STOP, &mut request)
                == NATIVE_CONTAINER_STATE_STOPPED
            {
                write_name_state(console, name, b"stopped\r\n");
            } else {
                write(console, b"container: stop rejected\r\n");
            }
        }
        b"fail" if words.next().is_none() => {
            if container_call(host, NATIVE_CONTAINER_INJECT_FAILURE, &mut request)
                == NATIVE_CONTAINER_STATE_FAILED
            {
                write_name_state(console, name, b"failed (contained)\r\n");
            } else {
                write(console, b"container: failure injection rejected\r\n");
            }
        }
        _ => write(console, b"container: invalid command\r\n"),
    }
}

fn request_for_name(name: &[u8]) -> NativeContainerRequestV0 {
    let mut request = NativeContainerRequestV0::zeroed();
    request.name_size = name.len() as u16;
    request.name[..name.len()].copy_from_slice(name);
    request
}

fn request_name(request: &NativeContainerRequestV0) -> &[u8] {
    request
        .name
        .get(..usize::from(request.name_size))
        .unwrap_or_default()
}

fn valid_container_name(name: &[u8]) -> bool {
    !name.is_empty()
        && name.len() <= NATIVE_CONTAINER_MAX_NAME_SIZE
        && name
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn set_request_value(request: &mut NativeContainerRequestV0, field: &[u8], value: &[u8]) -> bool {
    match (field, value) {
        (b"init", b"/sbin/init") => {
            request.init_size = value.len() as u16;
            request.init[..value.len()].copy_from_slice(value);
            true
        }
        (b"cpu", b"1") => {
            request.value = 1;
            true
        }
        (b"memory-mib", b"128") => {
            request.value = 128;
            true
        }
        (b"autostart", b"true") => {
            request.value = 1;
            true
        }
        _ => false,
    }
}

fn container_call(host: u64, operation: u64, request: &mut NativeContainerRequestV0) -> u64 {
    call(
        NATIVE_CONTAINER_CONTROL_OPERATION,
        host,
        operation,
        (request as *mut NativeContainerRequestV0) as u64,
    )
}

fn write_name_state(console: u64, name: &[u8], state: &[u8]) {
    write(console, name);
    write(console, b": ");
    write(console, state);
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
    let mut configuration = NativeContainerRequestV0::zeroed();
    let _ = container_call(host, NATIVE_CONTAINER_QUERY, &mut configuration);
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
                NATIVE_HOST_PROCESS_KIND_CONTAINER_INIT => request_name(&configuration),
                _ => b"unknown",
            },
        );
        if record.kind == NATIVE_HOST_PROCESS_KIND_CONTAINER_INIT {
            write(console, b":init");
        }
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

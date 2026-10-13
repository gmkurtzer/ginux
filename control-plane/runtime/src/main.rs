#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use ginux_command::{
    parse, write_error, write_help, Arguments, Arity, Command, HelpWriter, ParseOutcome,
};
use ginux_native_abi::{
    native_control_abi_is_compatible, NativeContainerRequestV0, NativeControlDescriptorV4,
    NativeHostInspectRecordV0, NativeHostInspectRequestV0, NativeRootDirectoryRequestV4,
    NATIVE_CALL_FAILED, NATIVE_CONSOLE_MAX_LINE_BYTES, NATIVE_CONSOLE_READ_LINE_OPERATION,
    NATIVE_CONSOLE_WRITE_OPERATION, NATIVE_CONTAINER_CONFIG_AUTOSTART, NATIVE_CONTAINER_CONFIG_CPU,
    NATIVE_CONTAINER_CONFIG_INIT, NATIVE_CONTAINER_CONFIG_MEMORY,
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

const MAX_LINE: usize = NATIVE_CONSOLE_MAX_LINE_BYTES as usize;
static CONTROLLER_IDENTITY: &[u8] = b"GINUX_CONTROLLER_V0";

const COMMAND_STATUS: u16 = 1;
const COMMAND_COMPONENTS: u16 = 2;
const COMMAND_LS: u16 = 3;
const COMMAND_PS: u16 = 4;
const COMMAND_SHUTDOWN: u16 = 5;
const COMMAND_CONTAINER_NEW: u16 = 6;
const COMMAND_CONTAINER_SET: u16 = 7;
const COMMAND_CONTAINER_STATUS: u16 = 8;
const COMMAND_CONTAINER_START: u16 = 9;
const COMMAND_CONTAINER_STOP: u16 = 10;
const COMMAND_CONTAINER_RESTART: u16 = 11;
const COMMAND_CONTAINER_FAIL: u16 = 12;
const COMMAND_CONTAINER_LIST: u16 = 13;

const CONTAINER_COMMANDS: &[Command<'static>] = &[
    Command::leaf(
        b"list",
        b"container list",
        b"List all containers",
        COMMAND_CONTAINER_LIST,
        Arity::exact(0),
    ),
    Command::leaf(
        b"new",
        b"container new NAME",
        b"Create a container configuration",
        COMMAND_CONTAINER_NEW,
        Arity::exact(1),
    ),
    Command::leaf(
        b"set",
        b"container set NAME FIELD=VALUE...",
        b"Set one or more container fields",
        COMMAND_CONTAINER_SET,
        Arity::at_least(2),
    ),
    Command::leaf(
        b"status",
        b"container status NAME",
        b"Show container state",
        COMMAND_CONTAINER_STATUS,
        Arity::exact(1),
    ),
    Command::leaf(
        b"start",
        b"container start NAME",
        b"Start a configured container",
        COMMAND_CONTAINER_START,
        Arity::exact(1),
    ),
    Command::leaf(
        b"stop",
        b"container stop NAME",
        b"Stop a running container",
        COMMAND_CONTAINER_STOP,
        Arity::exact(1),
    ),
    Command::leaf(
        b"restart",
        b"container restart NAME",
        b"Restart a configured container",
        COMMAND_CONTAINER_RESTART,
        Arity::exact(1),
    ),
    Command::leaf(
        b"fail",
        b"container fail NAME",
        b"Inject a contained container failure",
        COMMAND_CONTAINER_FAIL,
        Arity::exact(1),
    ),
];

const ROOT_COMMANDS: &[Command<'static>] = &[
    Command::leaf(
        b"help",
        b"help [COMMAND...]",
        b"Show command help",
        0,
        Arity::range(0, 3),
    ),
    Command::leaf(
        b"status",
        b"status",
        b"Show controller and host state",
        COMMAND_STATUS,
        Arity::exact(0),
    ),
    Command::leaf(
        b"components",
        b"components",
        b"Show embedded image components",
        COMMAND_COMPONENTS,
        Arity::exact(0),
    ),
    Command::leaf(
        b"ls",
        b"ls [PATH]",
        b"List the embedded measured root or /proc",
        COMMAND_LS,
        Arity::range(0, 1),
    ),
    Command::leaf(
        b"ps",
        b"ps",
        b"Show live host processes",
        COMMAND_PS,
        Arity::exact(0),
    ),
    Command::group(
        b"container",
        b"container COMMAND",
        b"Manage runtime containers",
        CONTAINER_COMMANDS,
    ),
    Command::leaf(
        b"shutdown",
        b"shutdown",
        b"Shut down Ginux cleanly",
        COMMAND_SHUTDOWN,
        Arity::exact(0),
    ),
];

const ROOT_COMMAND: Command<'static> = Command::group(
    b"ginux",
    b"ginux COMMAND",
    b"Ginux controller shell",
    ROOT_COMMANDS,
);

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
        let invocation = match parse(&ROOT_COMMAND, command) {
            Ok(ParseOutcome::Empty) => continue,
            Ok(ParseOutcome::Help(command)) => {
                let mut output = ConsoleHelpWriter::new(console);
                write_help(command, &mut output);
                output.finish();
                continue;
            }
            Ok(ParseOutcome::Invoke(invocation)) => invocation,
            Err(error) => {
                let mut output = ConsoleHelpWriter::new(console);
                write_error(error, &mut output);
                output.finish();
                continue;
            }
        };
        match invocation.id() {
            COMMAND_STATUS => {
                write(console, b"controller: running (ring 3)\r\n");
                write_container_summary(console, host);
                write(console, b"integrity: sealed\r\n");
            }
            COMMAND_COMPONENTS => {
                write(console, b"GINUX.EFI component catalog v0:\r\n");
                write(console, b"- kernel: active, ring 0\r\n");
                write(console, b"- controller: active, ring 3\r\n");
                write(console, b"- boot modules: embedded, sealed\r\n");
            }
            COMMAND_PS => show_processes(console, host),
            COMMAND_SHUTDOWN => {
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
            COMMAND_LS => {
                let path = invocation.arguments().first().unwrap_or_default();
                let path = path.strip_prefix(b"/").unwrap_or(path);
                if path == b"proc" {
                    list_proc_directory(console, host, b"");
                } else if let Some(path) = path.strip_prefix(b"proc/") {
                    list_proc_directory(console, host, path);
                } else {
                    list_directory(console, root, path);
                }
            }
            COMMAND_CONTAINER_NEW..=COMMAND_CONTAINER_FAIL => {
                handle_container_command(console, host, invocation.id(), invocation.arguments())
            }
            COMMAND_CONTAINER_LIST => show_container_list(console, host),
            _ => exit(NATIVE_SERVICE_EXIT_REJECTED),
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
        if state == NATIVE_CONTAINER_STATE_CREATED {
            b"created (incomplete)"
        } else {
            container_state_name(state)
        },
    );
    write(console, b"\r\n");
}

fn container_state_name(state: u64) -> &'static [u8] {
    match state {
        NATIVE_CONTAINER_STATE_RUNNING => b"running",
        NATIVE_CONTAINER_STATE_STOPPED => b"stopped",
        NATIVE_CONTAINER_STATE_FAILED => b"failed",
        NATIVE_CONTAINER_STATE_CREATED => b"created",
        NATIVE_CONTAINER_STATE_CONFIGURED => b"configured",
        _ => b"unknown",
    }
}

fn show_container_list(console: u64, host: u64) {
    write(console, b"NAME STATE INIT CPU MEM-MIB AUTO\r\n");
    let mut request = NativeContainerRequestV0::zeroed();
    let state = container_call(host, NATIVE_CONTAINER_QUERY, &mut request);
    if state == 0 {
        return;
    }
    if state == NATIVE_CALL_FAILED {
        write(console, b"container: list failed\r\n");
        return;
    }
    write(console, request_name(&request));
    write(console, b" ");
    write(console, container_state_name(state));
    write(console, b" ");
    write_container_configuration(console, &request);
}

fn write_container_configuration(console: u64, request: &NativeContainerRequestV0) {
    let configured = |flag| request.config_flags & flag != 0;
    if configured(NATIVE_CONTAINER_CONFIG_INIT) {
        let init = request
            .init
            .get(..usize::from(request.init_size))
            .unwrap_or_default();
        write(console, init);
    } else {
        write(console, b"-");
    }
    write(console, b" ");
    write(
        console,
        if configured(NATIVE_CONTAINER_CONFIG_CPU) {
            b"1"
        } else {
            b"-"
        },
    );
    write(console, b" ");
    write(
        console,
        if configured(NATIVE_CONTAINER_CONFIG_MEMORY) {
            b"128"
        } else {
            b"-"
        },
    );
    write(console, b" ");
    write(
        console,
        if configured(NATIVE_CONTAINER_CONFIG_AUTOSTART) {
            b"true\r\n"
        } else {
            b"-\r\n"
        },
    );
}

fn handle_container_command(console: u64, host: u64, command: u16, arguments: Arguments<'_>) {
    let name = arguments.first().unwrap_or_default();
    if !valid_container_name(name) {
        write(console, b"container: invalid name\r\n");
        return;
    }
    let mut request = request_for_name(name);
    match command {
        COMMAND_CONTAINER_NEW => {
            if container_call(host, NATIVE_CONTAINER_CREATE, &mut request)
                == NATIVE_CONTAINER_STATE_CREATED
            {
                write_name_state(console, name, b"created\r\n");
            } else {
                write(console, b"container: create rejected\r\n");
            }
        }
        COMMAND_CONTAINER_SET => {
            set_container_configuration(console, host, name, arguments);
        }
        COMMAND_CONTAINER_STATUS => {
            let state = container_call(host, NATIVE_CONTAINER_QUERY, &mut request);
            if state == NATIVE_CALL_FAILED {
                write(console, b"container: not found\r\n");
            } else {
                write(console, name);
                write(console, b": ");
                write_container_state(console, state);
            }
        }
        COMMAND_CONTAINER_START | COMMAND_CONTAINER_RESTART => {
            if container_call(host, NATIVE_CONTAINER_RESTART, &mut request)
                == NATIVE_CONTAINER_STATE_STOPPED
            {
                write_name_state(console, name, b"starting\r\n");
            } else {
                write(console, b"container: start rejected\r\n");
            }
        }
        COMMAND_CONTAINER_STOP => {
            if container_call(host, NATIVE_CONTAINER_STOP, &mut request)
                == NATIVE_CONTAINER_STATE_STOPPED
            {
                write_name_state(console, name, b"stopped\r\n");
            } else {
                write(console, b"container: stop rejected\r\n");
            }
        }
        COMMAND_CONTAINER_FAIL => {
            if container_call(host, NATIVE_CONTAINER_INJECT_FAILURE, &mut request)
                == NATIVE_CONTAINER_STATE_FAILED
            {
                write_name_state(console, name, b"failed (contained)\r\n");
            } else {
                write(console, b"container: failure injection rejected\r\n");
            }
        }
        _ => exit(NATIVE_SERVICE_EXIT_REJECTED),
    }
}

fn set_container_configuration(console: u64, host: u64, name: &[u8], arguments: Arguments<'_>) {
    let legacy = arguments.len() == 3 && !arguments.get(1).unwrap_or_default().contains(&b'=');
    let setting_count = if legacy { 1 } else { arguments.len() - 1 };
    let mut configured = 0_u16;
    for setting in arguments.skip(1).take(setting_count) {
        let (field, value) = if legacy {
            (setting, arguments.get(2).unwrap_or_default())
        } else if let Some(pair) = split_setting(setting) {
            pair
        } else {
            write(console, b"container: invalid setting\r\n");
            return;
        };
        let mut request = request_for_name(name);
        let Some((flag, _)) = prepare_setting(&mut request, field, value) else {
            write(console, b"container: invalid setting\r\n");
            return;
        };
        if configured & flag != 0 {
            write(console, b"container: duplicate setting\r\n");
            return;
        }
        configured |= flag;
    }

    let mut state = NATIVE_CALL_FAILED;
    for setting in arguments.skip(1).take(setting_count) {
        let (field, value) = if legacy {
            (setting, arguments.get(2).unwrap_or_default())
        } else {
            split_setting(setting).unwrap()
        };
        let mut request = request_for_name(name);
        let (_, operation) = prepare_setting(&mut request, field, value).unwrap();
        state = container_call(host, operation, &mut request);
        if !matches!(
            state,
            NATIVE_CONTAINER_STATE_CREATED | NATIVE_CONTAINER_STATE_CONFIGURED
        ) {
            write(console, b"container: setting rejected\r\n");
            return;
        }
    }
    write(console, name);
    write(
        console,
        if legacy || arguments.len() == 2 {
            b": setting accepted; "
        } else {
            b": settings accepted; "
        },
    );
    write_container_state(console, state);
}

fn split_setting(setting: &[u8]) -> Option<(&[u8], &[u8])> {
    let separator = setting.iter().position(|byte| *byte == b'=')?;
    let (field, value) = setting.split_at(separator);
    let value = value.get(1..)?;
    (!field.is_empty() && !value.is_empty() && !value.contains(&b'=')).then_some((field, value))
}

fn prepare_setting(
    request: &mut NativeContainerRequestV0,
    field: &[u8],
    value: &[u8],
) -> Option<(u16, u64)> {
    match (field, value) {
        (b"init", b"/sbin/init") => {
            request.init_size = value.len() as u16;
            request.init[..value.len()].copy_from_slice(value);
            Some((NATIVE_CONTAINER_CONFIG_INIT, NATIVE_CONTAINER_SET_INIT))
        }
        (b"cpu", b"1") => {
            request.value = 1;
            Some((NATIVE_CONTAINER_CONFIG_CPU, NATIVE_CONTAINER_SET_CPU))
        }
        (b"memory-mib", b"128") => {
            request.value = 128;
            Some((
                NATIVE_CONTAINER_CONFIG_MEMORY,
                NATIVE_CONTAINER_SET_MEMORY_MIB,
            ))
        }
        (b"autostart", b"true") => {
            request.value = 1;
            Some((
                NATIVE_CONTAINER_CONFIG_AUTOSTART,
                NATIVE_CONTAINER_SET_AUTOSTART,
            ))
        }
        _ => None,
    }
}

struct ConsoleHelpWriter {
    console: u64,
    bytes: [u8; 64],
    length: usize,
}

impl ConsoleHelpWriter {
    const fn new(console: u64) -> Self {
        Self {
            console,
            bytes: [0; 64],
            length: 0,
        }
    }

    fn finish(&mut self) {
        if self.length != 0 {
            write(self.console, &self.bytes[..self.length]);
            self.length = 0;
        }
    }
}

impl HelpWriter for ConsoleHelpWriter {
    fn write(&mut self, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            let available = self.bytes.len() - self.length;
            let count = available.min(bytes.len());
            self.bytes[self.length..self.length + count].copy_from_slice(&bytes[..count]);
            self.length += count;
            bytes = &bytes[count..];
            if self.length == self.bytes.len() {
                self.finish();
            }
        }
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

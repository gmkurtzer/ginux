#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use ginux_command::{
    complete, parse, write_error, write_help, Arguments, Arity, Command, HelpWriter, ParseOutcome,
};
use ginux_native_abi::{
    native_control_abi_is_compatible, NativeContainerRequestV2, NativeControlDescriptorV4,
    NativeHostInspectRecordV0, NativeHostInspectRequestV0, NativeRootDirectoryRequestV4,
    NATIVE_CALL_FAILED, NATIVE_CONSOLE_MAX_LINE_BYTES, NATIVE_CONSOLE_READ_BYTE_OPERATION,
    NATIVE_CONSOLE_READ_LINE_OPERATION, NATIVE_CONSOLE_WRITE_OPERATION, NATIVE_CONTAINER_ATTACH,
    NATIVE_CONTAINER_CONFIG_AUTOSTART, NATIVE_CONTAINER_CONFIG_CPU, NATIVE_CONTAINER_CONFIG_IMAGE,
    NATIVE_CONTAINER_CONFIG_INIT, NATIVE_CONTAINER_CONFIG_MEMORY,
    NATIVE_CONTAINER_CONTROL_OPERATION, NATIVE_CONTAINER_CREATE, NATIVE_CONTAINER_CURSOR_END,
    NATIVE_CONTAINER_HOST_SHUTDOWN, NATIVE_CONTAINER_INJECT_FAILURE, NATIVE_CONTAINER_LIMIT,
    NATIVE_CONTAINER_MAX_NAME_SIZE, NATIVE_CONTAINER_QUERY, NATIVE_CONTAINER_RESTART,
    NATIVE_CONTAINER_SET_AUTOSTART, NATIVE_CONTAINER_SET_CPU, NATIVE_CONTAINER_SET_IMAGE,
    NATIVE_CONTAINER_SET_INIT, NATIVE_CONTAINER_SET_MEMORY_MIB, NATIVE_CONTAINER_START_ATTACHED,
    NATIVE_CONTAINER_STATE_CONFIGURED, NATIVE_CONTAINER_STATE_CREATED,
    NATIVE_CONTAINER_STATE_FAILED, NATIVE_CONTAINER_STATE_RUNNING, NATIVE_CONTAINER_STATE_STOPPED,
    NATIVE_CONTAINER_STOP, NATIVE_CONTROL_DESCRIPTOR_OPERATION, NATIVE_CONTROL_DESCRIPTOR_SIZE,
    NATIVE_CONTROL_DISCOVERY_OPERATION, NATIVE_CONTROL_MAX_BUFFERED_CONSOLE_BYTES,
    NATIVE_HOST_INSPECT_OPERATION, NATIVE_HOST_INSPECT_RECORD_SIZE,
    NATIVE_HOST_INSPECT_REQUEST_SIZE, NATIVE_HOST_INSPECT_STATE_READY,
    NATIVE_HOST_INSPECT_STATE_STOPPING, NATIVE_HOST_INSPECT_VERSION,
    NATIVE_HOST_PROCESS_KIND_CONTAINER_INIT, NATIVE_HOST_PROCESS_KIND_CONTROLLER,
    NATIVE_HOST_PROC_ENUMERATE, NATIVE_HOST_PROC_STATUS, NATIVE_LINUX_ABI_TEST_COUNT,
    NATIVE_LINUX_ABI_TEST_OPERATION, NATIVE_NETWORK_APPLY_VERSION, NATIVE_NETWORK_DEVICE_APPLY,
    NATIVE_NETWORK_DEVICE_DHCP, NATIVE_NETWORK_DEVICE_OPERATION, NATIVE_NETWORK_DEVICE_PING,
    NATIVE_NETWORK_DEVICE_QUERY_DHCP, NATIVE_NETWORK_DEVICE_QUERY_DNS,
    NATIVE_NETWORK_DEVICE_QUERY_PING, NATIVE_NETWORK_DEVICE_QUERY_SNAPSHOT,
    NATIVE_NETWORK_DEVICE_SET_DNS, NATIVE_NETWORK_DEVICE_SET_LINK, NATIVE_NETWORK_SNAPSHOT_SIZE,
    NATIVE_NETWORK_SNAPSHOT_VERSION, NATIVE_ROOT_DIRECTORY_RECORD_HEADER_SIZE,
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
const COMMAND_NETWORK_STATUS: u16 = 14;
const COMMAND_NETWORK_SET: u16 = 15;
const COMMAND_NETWORK_DOWN: u16 = 16;
const COMMAND_PING: u16 = 17;
const COMMAND_NETWORK_LIST: u16 = 19;
const COMMAND_NETWORK_START: u16 = 20;
const COMMAND_NETWORK_STOP: u16 = 21;
const COMMAND_NETWORK_RESTART: u16 = 22;
const COMMAND_TEST_LINUX: u16 = 23;
const COMMAND_CONTAINER_ATTACH: u16 = 24;

const LINUX_ABI_TESTS: &[&[u8]] = &[
    b"syscall-entry",
    b"elf64-loader",
    b"initial-stack-and-auxv",
    b"dynamic-loader",
    b"tls",
    b"process-and-thread",
    b"signals",
    b"memory-management",
    b"descriptors-and-events",
    b"vfs-and-path-lookup",
    b"procfs",
    b"devfs-and-terminal",
    b"tmpfs",
    b"credentials-and-user-namespaces",
    b"clocks-and-timers",
    b"ioctl-profile",
    b"sysfs",
    b"mount-and-namespace-api",
    b"socket-api",
    b"cgroups",
    b"seccomp-audit-and-lsm",
    b"vdso-and-vvar",
    b"linux-module-userspace-api",
];

const TEST_COMMANDS: &[Command<'static>] = &[Command::leaf(
    b"linux",
    b"test linux",
    b"Test Linux userspace interfaces",
    COMMAND_TEST_LINUX,
    Arity::exact(0),
)];

const NETWORK_COMMANDS: &[Command<'static>] = &[
    Command::leaf(
        b"list",
        b"network list",
        b"List network devices and their state",
        COMMAND_NETWORK_LIST,
        Arity::exact(0),
    ),
    Command::leaf(
        b"status",
        b"network status DEVICE",
        b"Show a network device configuration",
        COMMAND_NETWORK_STATUS,
        Arity::exact(1),
    ),
    Command::leaf(
        b"set",
        b"network set DEVICE {dhcp|dns ADDRESS|static ADDRESS/PREFIX GATEWAY}",
        b"Configure a network device",
        COMMAND_NETWORK_SET,
        Arity::range(2, 4),
    ),
    Command::leaf(
        b"start",
        b"network start DEVICE",
        b"Administratively start a network device",
        COMMAND_NETWORK_START,
        Arity::exact(1),
    ),
    Command::leaf(
        b"stop",
        b"network stop DEVICE",
        b"Administratively stop a network device",
        COMMAND_NETWORK_STOP,
        Arity::exact(1),
    ),
    Command::leaf(
        b"restart",
        b"network restart DEVICE",
        b"Restart a network device without losing its configuration",
        COMMAND_NETWORK_RESTART,
        Arity::exact(1),
    ),
    Command::leaf(
        b"ping",
        b"network ping DEVICE ADDRESS",
        b"Send one ICMP echo through a network device",
        COMMAND_PING,
        Arity::exact(2),
    ),
    Command::leaf(
        b"down",
        b"network down DEVICE",
        b"Alias for network stop",
        COMMAND_NETWORK_DOWN,
        Arity::exact(1),
    ),
];

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
        b"container start NAME [--attach]",
        b"Start a configured container, optionally attached",
        COMMAND_CONTAINER_START,
        Arity::range(1, 2),
    ),
    Command::leaf(
        b"attach",
        b"container attach NAME",
        b"Attach to a running container; Ctrl-] detaches",
        COMMAND_CONTAINER_ATTACH,
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
    Command::group(
        b"network",
        b"network COMMAND",
        b"Manage network devices",
        NETWORK_COMMANDS,
    ),
    Command::group(
        b"test",
        b"test ENVIRONMENT",
        b"Test a userspace compatibility environment",
        TEST_COMMANDS,
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
    mov rcx, r8
    call ginux_controller_main
    ud2
"#
);

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    exit(NATIVE_SERVICE_EXIT_REJECTED)
}

#[no_mangle]
extern "C" fn ginux_controller_main(console: u64, host: u64, root: u64, network: u64) -> ! {
    let identity = unsafe { core::ptr::read_volatile(&CONTROLLER_IDENTITY) };
    if identity != b"GINUX_CONTROLLER_V0"
        || console == 0
        || host == 0
        || root == 0
        || network == 0
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
    if network_snapshot(network).is_some_and(|snapshot| snapshot[3] == 0) {
        write(console, b"eth0: automatic DHCP configuration\r\n");
        set_dhcp_network(console, network);
    }
    loop {
        write(console, b"ginux> ");
        let length = read_line(console, &mut line);
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
            COMMAND_CONTAINER_ATTACH => {
                handle_container_command(console, host, invocation.id(), invocation.arguments())
            }
            COMMAND_NETWORK_LIST => show_network_list(console, network),
            COMMAND_NETWORK_STATUS => show_network_status(console, network, invocation.arguments()),
            COMMAND_NETWORK_SET => set_network(console, network, invocation.arguments()),
            COMMAND_NETWORK_START => {
                set_network_link(console, network, invocation.arguments(), true)
            }
            COMMAND_NETWORK_STOP | COMMAND_NETWORK_DOWN => {
                set_network_link(console, network, invocation.arguments(), false)
            }
            COMMAND_NETWORK_RESTART => restart_network(console, network, invocation.arguments()),
            COMMAND_PING => ping(console, network, invocation.arguments()),
            COMMAND_TEST_LINUX => test_linux_userspace(console, host),
            _ => exit(NATIVE_SERVICE_EXIT_REJECTED),
        }
    }
}

fn read_line(console: u64, line: &mut [u8; MAX_LINE]) -> usize {
    let mut length = 0;
    loop {
        let byte = read_console_byte(console);
        match byte {
            b'\r' | b'\n' => {
                write(console, b"\r\n");
                return length;
            }
            b'\t' => {
                let (input, output) = line.split_at_mut(length);
                let added = complete(&ROOT_COMMAND, input, output);
                if added != 0 {
                    write(console, &line[length..length + added]);
                    length += added;
                }
            }
            0x08 | 0x7f if length != 0 => {
                length -= 1;
                write(console, b"\x08 \x08");
            }
            byte @ 0x20..=0x7e if length < line.len() => {
                line[length] = byte;
                length += 1;
                write(console, &[byte]);
            }
            _ => {}
        }
    }
}

fn read_console_byte(console: u64) -> u8 {
    const RETRY_LIMIT: usize = 3;
    for _ in 0..RETRY_LIMIT {
        if let Ok(byte) = u8::try_from(call(NATIVE_CONSOLE_READ_BYTE_OPERATION, console, 0, 0)) {
            return byte;
        }
    }
    exit(NATIVE_SERVICE_EXIT_REJECTED)
}

fn test_linux_userspace(console: u64, host: u64) {
    let results = call(NATIVE_LINUX_ABI_TEST_OPERATION, host, 0, 0);
    let results = if results == NATIVE_CALL_FAILED {
        0
    } else {
        results
    };
    let mut passed = 0_u64;
    let mut output = ConsoleHelpWriter::new(console);
    output.write(b"Linux userspace interface tests (bounded profile):\r\n");
    for (index, name) in LINUX_ABI_TESTS.iter().enumerate() {
        let pass = results & (1 << index) != 0;
        output.write(if pass { b"PASS " } else { b"FAIL " });
        output.write(name);
        output.write(b"\r\n");
        passed += u64::from(pass);
    }
    let total = LINUX_ABI_TESTS.len() as u64;
    output.write(b"Summary: ");
    write_help_decimal(&mut output, passed);
    output.write(b" passed, ");
    write_help_decimal(&mut output, total - passed);
    output.write(b" failed, ");
    write_help_decimal(&mut output, total);
    output.write(b" total\r\n");
    output.write(
        b"PASS covers the current bounded Linux profile; FAIL is unsupported or unverified.\r\n",
    );
    output.finish();
}

fn write_help_decimal(output: &mut impl HelpWriter, mut value: u64) {
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
    output.write(&bytes[start..]);
}

fn ping(console: u64, network: u64, arguments: Arguments<'_>) {
    let device = arguments.first();
    if !is_eth0(device) && !is_loopback(device) {
        write(console, b"network: device not found\r\n");
        return;
    }
    let address_text = arguments.get(1).unwrap_or_default();
    let Some(address) = parse_ipv4(address_text) else {
        write(console, b"ping: invalid address\r\n");
        return;
    };
    write(console, b"PING ");
    write(console, address_text);
    write(console, b"\r\n");
    if is_loopback(device) {
        if address.to_be_bytes()[0] == 127 {
            write(console, b"64 bytes from ");
            write_ipv4(console, address);
            write(console, b": icmp_seq=1 ttl=64\r\n");
        } else {
            write(console, b"ping: target is not a loopback address\r\n");
        }
        return;
    }
    if call(
        NATIVE_NETWORK_DEVICE_OPERATION,
        network,
        NATIVE_NETWORK_DEVICE_PING,
        u64::from(address),
    ) == NATIVE_CALL_FAILED
    {
        write(
            console,
            b"ping: network unavailable or target could not be reached\r\n",
        );
        return;
    }
    match call(
        NATIVE_NETWORK_DEVICE_OPERATION,
        network,
        NATIVE_NETWORK_DEVICE_QUERY_PING,
        0,
    ) {
        1 => {
            write(console, b"64 bytes from ");
            write_ipv4(console, address);
            write(console, b": icmp_seq=1 ttl=64\r\n");
        }
        2 => write(console, b"ping: request timed out\r\n"),
        _ => write(console, b"ping: result unavailable\r\n"),
    }
}

fn show_network_list(console: u64, network: u64) {
    let Some(snapshot) = network_snapshot(network) else {
        write(console, b"network: status failed\r\n");
        return;
    };
    write(
        console,
        b"DEVICE  DOMAIN   CONNECTED  STATE  CONFIGURATION  ADDRESS          GATEWAY          DNS\r\n",
    );
    write_eth0_status(console, network, snapshot, false);
    write_loopback_status(console, false);
}

fn show_network_status(console: u64, network: u64, arguments: Arguments<'_>) {
    if is_loopback(arguments.first()) {
        write(
            console,
            b"DEVICE  DOMAIN   CONNECTED  STATE  CONFIGURATION  ADDRESS          GATEWAY          DNS             GENERATION\r\n",
        );
        write_loopback_status(console, true);
        return;
    }
    if !is_eth0(arguments.first()) {
        write(console, b"network: device not found\r\n");
        return;
    }
    let Some(snapshot) = network_snapshot(network) else {
        write(console, b"network: status failed\r\n");
        return;
    };
    write(
        console,
        b"DEVICE  DOMAIN   CONNECTED  STATE  CONFIGURATION  ADDRESS          GATEWAY          DNS             GENERATION\r\n",
    );
    write_eth0_status(console, network, snapshot, true);
}

fn write_eth0_status(console: u64, network: u64, snapshot: [u64; 7], generation: bool) {
    let mut row = OutputLine::new();
    row.padded(b"eth0", 8);
    row.padded(b"control", 9);
    row.padded(b"yes", 11);
    row.padded(if snapshot[2] == 1 { b"up" } else { b"down" }, 7);
    row.padded(
        if snapshot[3] == 0 {
            b"unconfigured"
        } else {
            b"configured"
        },
        15,
    );
    if snapshot[3] == 0 {
        row.padded(b"-", 17);
        row.padded(b"-", 17);
    } else {
        let mut address = [0_u8; 18];
        let mut length = format_ipv4(&mut address, snapshot[3] as u32);
        address[length] = b'/';
        length += 1;
        length += append_decimal(&mut address[length..], snapshot[3] >> 32);
        row.padded(&address[..length], 17);
        let mut gateway = [0_u8; 15];
        let length = format_ipv4(&mut gateway, snapshot[4] as u32);
        row.padded(&gateway[..length], 17);
    }
    let dns = call(
        NATIVE_NETWORK_DEVICE_OPERATION,
        network,
        NATIVE_NETWORK_DEVICE_QUERY_DNS,
        0,
    );
    let mut dns_address = [0_u8; 15];
    let dns = if dns == NATIVE_CALL_FAILED || dns == 0 {
        b"-" as &[u8]
    } else {
        let length = format_ipv4(&mut dns_address, dns as u32);
        &dns_address[..length]
    };
    if generation {
        row.padded(dns, 16);
        row.decimal(snapshot[1]);
    } else {
        row.push(dns);
    }
    row.push(b"\r\n");
    row.write(console);
}

fn write_loopback_status(console: u64, generation: bool) {
    let mut row = OutputLine::new();
    row.padded(b"lo", 8);
    row.padded(b"control", 9);
    row.padded(b"yes", 11);
    row.padded(b"up", 7);
    row.padded(b"configured", 15);
    row.padded(b"127.0.0.1/8", 17);
    row.padded(b"-", 17);
    row.push(b"-");
    if generation {
        row.spaces(16);
        row.push(b"0");
    }
    row.push(b"\r\n");
    row.write(console);
}

fn set_network(console: u64, network: u64, arguments: Arguments<'_>) {
    if is_loopback(arguments.first()) {
        write(console, b"network: loopback is automatic and immutable\r\n");
        return;
    }
    if !is_eth0(arguments.first()) {
        write(console, b"network: device not found\r\n");
        return;
    }
    match arguments.get(1) {
        Some(b"dhcp") if arguments.len() == 2 => set_dhcp_network(console, network),
        Some(b"dns") if arguments.len() == 3 => {
            set_network_dns(console, network, arguments.get(2))
        }
        Some(b"static") if arguments.len() == 4 => {
            set_static_network(console, network, arguments.get(2), arguments.get(3))
        }
        _ => write(
            console,
            b"network: usage: network set DEVICE {dhcp|dns ADDRESS|static ADDRESS/PREFIX GATEWAY}\r\n",
        ),
    }
}

fn set_static_network(console: u64, network: u64, address: Option<&[u8]>, gateway: Option<&[u8]>) {
    let Some((address, prefix)) = address.and_then(parse_ipv4_prefix) else {
        write(console, b"network: invalid address or prefix\r\n");
        return;
    };
    let Some(gateway) = gateway.and_then(parse_ipv4) else {
        write(console, b"network: invalid gateway\r\n");
        return;
    };
    let Some(snapshot) = network_snapshot(network) else {
        write(console, b"network: status failed\r\n");
        return;
    };
    let request = [
        NATIVE_NETWORK_APPLY_VERSION,
        snapshot[1],
        1,
        u64::from(address) | (u64::from(prefix) << 32),
        u64::from(gateway),
        0,
        0,
        0,
    ];
    let generation = call(
        NATIVE_NETWORK_DEVICE_OPERATION,
        network,
        NATIVE_NETWORK_DEVICE_APPLY,
        request.as_ptr() as u64,
    );
    if generation == NATIVE_CALL_FAILED {
        write(console, b"network: configuration rejected\r\n");
        return;
    }
    write(console, b"eth0: static network configured\r\n");
}

fn set_dhcp_network(console: u64, network: u64) {
    write(console, b"eth0: requesting DHCP lease\r\n");
    if call(
        NATIVE_NETWORK_DEVICE_OPERATION,
        network,
        NATIVE_NETWORK_DEVICE_DHCP,
        0,
    ) == NATIVE_CALL_FAILED
        || call(
            NATIVE_NETWORK_DEVICE_OPERATION,
            network,
            NATIVE_NETWORK_DEVICE_QUERY_DHCP,
            0,
        ) != 1
    {
        write(console, b"network: DHCP failed\r\n");
        return;
    }
    let Some(snapshot) = network_snapshot(network) else {
        write(console, b"network: DHCP status failed\r\n");
        return;
    };
    write(console, b"eth0: DHCP configured ");
    write_ipv4(console, snapshot[3] as u32);
    write(console, b"/");
    write_decimal(console, snapshot[3] >> 32);
    write(console, b" via ");
    write_ipv4(console, snapshot[4] as u32);
    write(console, b" DNS ");
    write_network_dns(console, network);
    write(console, b"\r\n");
}

fn set_network_dns(console: u64, network: u64, address: Option<&[u8]>) {
    let address = match address {
        Some(b"none") => 0,
        Some(address) => {
            let Some(address) = parse_ipv4(address) else {
                write(console, b"network: invalid DNS address\r\n");
                return;
            };
            address
        }
        None => {
            write(console, b"network: missing DNS address\r\n");
            return;
        }
    };
    if call(
        NATIVE_NETWORK_DEVICE_OPERATION,
        network,
        NATIVE_NETWORK_DEVICE_SET_DNS,
        u64::from(address),
    ) == NATIVE_CALL_FAILED
    {
        write(console, b"network: DNS configuration rejected\r\n");
        return;
    }
    write(console, b"eth0: DNS configured\r\n");
}

fn write_network_dns(console: u64, network: u64) {
    let dns = call(
        NATIVE_NETWORK_DEVICE_OPERATION,
        network,
        NATIVE_NETWORK_DEVICE_QUERY_DNS,
        0,
    );
    if dns == NATIVE_CALL_FAILED || dns == 0 {
        write(console, b"-");
    } else {
        write_ipv4(console, dns as u32);
    }
}

fn set_network_link(console: u64, network: u64, arguments: Arguments<'_>, start: bool) {
    if is_loopback(arguments.first()) {
        write(console, b"network: loopback is automatic and immutable\r\n");
        return;
    }
    if !is_eth0(arguments.first()) {
        write(console, b"network: device not found\r\n");
        return;
    }
    if call(
        NATIVE_NETWORK_DEVICE_OPERATION,
        network,
        NATIVE_NETWORK_DEVICE_SET_LINK,
        u64::from(start),
    ) == NATIVE_CALL_FAILED
    {
        write(console, b"network: state change rejected\r\n");
        return;
    }
    write(
        console,
        if start {
            b"eth0: started\r\n"
        } else {
            b"eth0: stopped\r\n"
        },
    );
}

fn restart_network(console: u64, network: u64, arguments: Arguments<'_>) {
    if is_loopback(arguments.first()) {
        write(console, b"network: loopback is automatic and immutable\r\n");
        return;
    }
    if !is_eth0(arguments.first()) {
        write(console, b"network: device not found\r\n");
        return;
    }
    if call(
        NATIVE_NETWORK_DEVICE_OPERATION,
        network,
        NATIVE_NETWORK_DEVICE_SET_LINK,
        0,
    ) == NATIVE_CALL_FAILED
        || call(
            NATIVE_NETWORK_DEVICE_OPERATION,
            network,
            NATIVE_NETWORK_DEVICE_SET_LINK,
            1,
        ) == NATIVE_CALL_FAILED
    {
        write(console, b"network: restart rejected\r\n");
        return;
    }
    write(console, b"eth0: restarted\r\n");
}

fn is_eth0(device: Option<&[u8]>) -> bool {
    device == Some(b"eth0")
}

fn is_loopback(device: Option<&[u8]>) -> bool {
    device == Some(b"lo")
}

fn network_snapshot(network: u64) -> Option<[u64; 7]> {
    let mut snapshot = [0_u64; 7];
    let result = call(
        NATIVE_NETWORK_DEVICE_OPERATION,
        network,
        NATIVE_NETWORK_DEVICE_QUERY_SNAPSHOT,
        snapshot.as_mut_ptr() as u64,
    );
    (result == NATIVE_NETWORK_SNAPSHOT_SIZE && snapshot[0] == NATIVE_NETWORK_SNAPSHOT_VERSION)
        .then_some(snapshot)
}

fn parse_ipv4_prefix(bytes: &[u8]) -> Option<(u32, u8)> {
    let separator = bytes.iter().position(|byte| *byte == b'/')?;
    let address = parse_ipv4(&bytes[..separator])?;
    let prefix = u8::try_from(parse_decimal(bytes.get(separator + 1..)?)?).ok()?;
    (prefix <= 32).then_some((address, prefix))
}

fn parse_ipv4(bytes: &[u8]) -> Option<u32> {
    let mut octets = [0_u8; 4];
    let mut parts = bytes.split(|byte| *byte == b'.');
    for octet in &mut octets {
        *octet = u8::try_from(parse_decimal(parts.next()?)?).ok()?;
    }
    parts.next().is_none().then_some(u32::from_be_bytes(octets))
}

fn write_ipv4(console: u64, address: u32) {
    let mut bytes = [0_u8; 15];
    let length = format_ipv4(&mut bytes, address);
    write(console, &bytes[..length]);
}

fn format_ipv4(bytes: &mut [u8], address: u32) -> usize {
    let mut length = 0;
    for (index, octet) in address.to_be_bytes().iter().enumerate() {
        if index != 0 {
            bytes[length] = b'.';
            length += 1;
        }
        length += append_decimal(&mut bytes[length..], u64::from(*octet));
    }
    length
}

fn append_decimal(output: &mut [u8], mut value: u64) -> usize {
    let mut reversed = [0_u8; 20];
    let mut count = 0;
    loop {
        reversed[count] = b'0' + (value % 10) as u8;
        count += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for index in 0..count {
        output[index] = reversed[count - index - 1];
    }
    count
}

fn write_container_summary(console: u64, host: u64) {
    let mut count = 0;
    let mut cursor = 0;
    while let Some((_, request)) = next_container(host, cursor) {
        count += 1;
        if request.value == NATIVE_CONTAINER_CURSOR_END {
            break;
        }
        cursor = request.value;
    }
    if count == 0 {
        write(console, b"containers: 0 configured\r\n");
        return;
    }
    write(console, b"containers: ");
    write_decimal(console, count);
    write(console, b" configured\r\n");
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
    write(
        console,
        b"NAME            STATE       INIT                  CPU  MEM-MIB  AUTO   IMAGE\r\n",
    );
    let mut cursor = 0;
    loop {
        let Some((state, request)) = next_container(host, cursor) else {
            return;
        };
        let mut row = OutputLine::new();
        row.padded(request_name(&request), 16);
        row.padded(container_state_name(state), 12);
        append_container_configuration(&mut row, &request);
        row.push(b"\r\n");
        row.write(console);
        if request.value == NATIVE_CONTAINER_CURSOR_END {
            return;
        }
        if request.value <= cursor {
            write(console, b"container: list cursor failed\r\n");
            return;
        }
        cursor = request.value;
    }
}

fn next_container(host: u64, cursor: u64) -> Option<(u64, NativeContainerRequestV2)> {
    let mut request = NativeContainerRequestV2::zeroed();
    request.value = cursor;
    let state = container_call(host, NATIVE_CONTAINER_QUERY, &mut request);
    (state != 0 && state != NATIVE_CALL_FAILED).then_some((state, request))
}

fn append_container_configuration(row: &mut OutputLine, request: &NativeContainerRequestV2) {
    let configured = |flag| request.config_flags & flag != 0;
    if configured(NATIVE_CONTAINER_CONFIG_INIT) {
        let init = request
            .init
            .get(..usize::from(request.init_size))
            .unwrap_or_default();
        row.padded(init, 22);
    } else {
        row.padded(b"-", 22);
    }
    if configured(NATIVE_CONTAINER_CONFIG_CPU) {
        row.decimal_padded(u64::from(request.cpu_count), 5);
    } else {
        row.padded(b"-", 5);
    }
    if configured(NATIVE_CONTAINER_CONFIG_MEMORY) {
        row.decimal_padded(request.memory_mib, 9);
    } else {
        row.padded(b"-", 9);
    }
    row.padded(
        if configured(NATIVE_CONTAINER_CONFIG_AUTOSTART) {
            b"true"
        } else {
            b"false"
        },
        7,
    );
    if configured(NATIVE_CONTAINER_CONFIG_IMAGE) {
        row.push(
            request
                .image
                .get(..usize::from(request.image_size))
                .unwrap_or_default(),
        );
    } else {
        row.push(b"-");
    }
}

fn handle_container_command(console: u64, host: u64, command: u16, arguments: Arguments<'_>) {
    let name = arguments.first().unwrap_or_default();
    if !valid_container_name(name) {
        write(console, b"container: invalid name\r\n");
        return;
    }
    if command == COMMAND_CONTAINER_START
        && arguments
            .get(1)
            .is_some_and(|argument| argument != b"--attach")
    {
        write(console, b"container: invalid option\r\n");
        return;
    }
    let mut request = request_for_name(name);
    match command {
        COMMAND_CONTAINER_NEW => {
            if container_call(host, NATIVE_CONTAINER_CREATE, &mut request)
                == NATIVE_CONTAINER_STATE_CREATED
            {
                write_name_state(console, name, b"created (image required)\r\n");
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
            let operation =
                if command == COMMAND_CONTAINER_START && arguments.get(1) == Some(b"--attach") {
                    NATIVE_CONTAINER_START_ATTACHED
                } else {
                    NATIVE_CONTAINER_RESTART
                };
            if container_call(host, operation, &mut request) == NATIVE_CALL_FAILED {
                write(console, b"container: start rejected (image not ready)\r\n");
                return;
            }
            let state = container_call(host, NATIVE_CONTAINER_QUERY, &mut request);
            if state == NATIVE_CONTAINER_STATE_RUNNING {
                write_name_state(console, name, b"running (detached)\r\n");
            } else if state == NATIVE_CONTAINER_STATE_STOPPED {
                write_name_state(console, name, b"stopped\r\n");
            } else if state == NATIVE_CONTAINER_STATE_FAILED {
                write_name_state(console, name, b"failed\r\n");
            } else {
                write(console, b"container: start failed\r\n");
            }
        }
        COMMAND_CONTAINER_ATTACH => {
            if container_call(host, NATIVE_CONTAINER_ATTACH, &mut request) == NATIVE_CALL_FAILED {
                write(
                    console,
                    b"container: attach rejected (container is not running)\r\n",
                );
                return;
            }
            let state = container_call(host, NATIVE_CONTAINER_QUERY, &mut request);
            if state == NATIVE_CONTAINER_STATE_RUNNING {
                write_name_state(console, name, b"running (detached)\r\n");
            } else if state == NATIVE_CONTAINER_STATE_STOPPED {
                write_name_state(console, name, b"stopped\r\n");
            } else if state == NATIVE_CONTAINER_STATE_FAILED {
                write_name_state(console, name, b"failed\r\n");
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
    request: &mut NativeContainerRequestV2,
    field: &[u8],
    value: &[u8],
) -> Option<(u16, u64)> {
    match (field, value) {
        (b"init", value)
            if value.starts_with(b"/")
                && value.len() <= request.init.len()
                && value.iter().all(|byte| byte.is_ascii_graphic()) =>
        {
            request.init_size = value.len() as u16;
            request.init[..value.len()].copy_from_slice(value);
            Some((NATIVE_CONTAINER_CONFIG_INIT, NATIVE_CONTAINER_SET_INIT))
        }
        (b"cpu", value) => {
            request.cpu_count = u16::try_from(parse_decimal(value)?).ok()?;
            if request.cpu_count == 0 {
                return None;
            }
            Some((NATIVE_CONTAINER_CONFIG_CPU, NATIVE_CONTAINER_SET_CPU))
        }
        (b"memory-mib", value) => {
            request.memory_mib = parse_decimal(value)?;
            if request.memory_mib == 0 {
                return None;
            }
            Some((
                NATIVE_CONTAINER_CONFIG_MEMORY,
                NATIVE_CONTAINER_SET_MEMORY_MIB,
            ))
        }
        (b"autostart", value @ (b"true" | b"false")) => {
            request.value = u64::from(value == b"true");
            Some((
                NATIVE_CONTAINER_CONFIG_AUTOSTART,
                NATIVE_CONTAINER_SET_AUTOSTART,
            ))
        }
        (b"image", value) if value.iter().all(|byte| byte.is_ascii_graphic()) => {
            if value.len() > request.image.len() {
                return None;
            }
            request.image_size = value.len() as u16;
            request.image[..value.len()].copy_from_slice(value);
            Some((NATIVE_CONTAINER_CONFIG_IMAGE, NATIVE_CONTAINER_SET_IMAGE))
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

fn request_for_name(name: &[u8]) -> NativeContainerRequestV2 {
    let mut request = NativeContainerRequestV2::zeroed();
    request.name_size = name.len() as u16;
    request.name[..name.len()].copy_from_slice(name);
    request
}

fn request_name(request: &NativeContainerRequestV2) -> &[u8] {
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

fn container_call(host: u64, operation: u64, request: &mut NativeContainerRequestV2) -> u64 {
    call(
        NATIVE_CONTAINER_CONTROL_OPERATION,
        host,
        operation,
        (request as *mut NativeContainerRequestV2) as u64,
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
    for chunk in bytes.chunks(NATIVE_CONTROL_MAX_BUFFERED_CONSOLE_BYTES as usize) {
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
    write(console, b"PID    PPID   STATE  NAME\r\n");
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
        let container = (record.kind == NATIVE_HOST_PROCESS_KIND_CONTAINER_INIT)
            .then(|| container_for_pid(host, record.pid))
            .flatten();
        let mut row = OutputLine::new();
        row.decimal_padded(record.pid, 7);
        row.decimal_padded(record.parent_pid, 7);
        row.padded(
            match record.state {
                NATIVE_HOST_INSPECT_STATE_READY => b"R",
                NATIVE_HOST_INSPECT_STATE_STOPPING => b"S",
                _ => b"unknown",
            },
            7,
        );
        row.push(match record.kind {
            NATIVE_HOST_PROCESS_KIND_CONTROLLER => b"controller",
            NATIVE_HOST_PROCESS_KIND_CONTAINER_INIT => {
                container.as_ref().map(request_name).unwrap_or(b"unknown")
            }
            _ => b"unknown",
        });
        if record.kind == NATIVE_HOST_PROCESS_KIND_CONTAINER_INIT {
            row.push(b":init");
        }
        row.push(b"\r\n");
        row.write(console);
        cursor = directory_record.next_cursor;
    }
}

fn container_for_pid(host: u64, pid: u64) -> Option<NativeContainerRequestV2> {
    let slot = pid.checked_sub(100)?;
    if slot >= NATIVE_CONTAINER_LIMIT as u64 {
        return None;
    }
    let (state, request) = next_container(host, slot)?;
    (state == NATIVE_CONTAINER_STATE_RUNNING).then_some(request)
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

struct OutputLine {
    bytes: [u8; MAX_LINE],
    length: usize,
}

impl OutputLine {
    const fn new() -> Self {
        Self {
            bytes: [0; MAX_LINE],
            length: 0,
        }
    }

    fn push(&mut self, value: &[u8]) {
        let Some(end) = self.length.checked_add(value.len()) else {
            exit(NATIVE_SERVICE_EXIT_REJECTED);
        };
        let Some(output) = self.bytes.get_mut(self.length..end) else {
            exit(NATIVE_SERVICE_EXIT_REJECTED);
        };
        output.copy_from_slice(value);
        self.length = end;
    }

    fn spaces(&mut self, count: usize) {
        const SPACES: &[u8; 32] = b"                                ";
        let mut remaining = count;
        while remaining != 0 {
            let length = remaining.min(SPACES.len());
            self.push(&SPACES[..length]);
            remaining -= length;
        }
    }

    fn padded(&mut self, value: &[u8], width: usize) {
        self.push(value);
        self.spaces(width.saturating_sub(value.len()).max(1));
    }

    fn decimal(&mut self, value: u64) {
        let mut bytes = [0_u8; 20];
        let length = append_decimal(&mut bytes, value);
        self.push(&bytes[..length]);
    }

    fn decimal_padded(&mut self, value: u64, width: usize) {
        let mut bytes = [0_u8; 20];
        let length = append_decimal(&mut bytes, value);
        self.padded(&bytes[..length], width);
    }

    fn write(&self, console: u64) {
        write(console, &self.bytes[..self.length]);
    }
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
const _: () = assert!(LINUX_ABI_TESTS.len() == NATIVE_LINUX_ABI_TEST_COUNT as usize);

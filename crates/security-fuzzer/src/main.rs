use ginux_contract::{
    resource_selector_matches, selector_matches, validate_resource_selector, validate_selector,
    AuditBudget,
};
use ginux_core::{
    balanced_arguments_allowed, balanced_surface, decide, decide_balanced, decide_systemd,
};
use ginux_devices::{BlockBackend, DeviceError, DeviceName, DeviceView, Partition};
use ginux_instance::InstanceConfig;
use ginux_kernel_views::SysFs;
use ginux_runtime::AddressSpace;
use ginux_vfs::Index;
use std::env;
use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::process;
use std::time::{Duration, Instant};

const MAX_INPUT: usize = 4096;
const CONFIG_SEED: &[u8] = include_bytes!("../../../tests/fuzz-corpus/config/minimal.toml");
const SELECTOR_SEED: &[u8] = include_bytes!("../../../tests/fuzz-corpus/selector/basic.txt");
const ELF_SEED: &[u8] = include_bytes!("../../../tests/fuzz-corpus/elf/header.bin");
const VFS_SEED: &[u8] = include_bytes!("../../../tests/fuzz-corpus/vfs-index/header.bin");
const PATH_SEED: &[u8] = include_bytes!("../../../tests/fuzz-corpus/kernel-path/basic.txt");
const DEVICE_SEED: &[u8] = include_bytes!("../../../tests/fuzz-corpus/device/basic.txt");

fn main() {
    let arguments: Vec<String> = env::args().collect();
    if arguments.len() != 4 {
        eprintln!("usage: security-fuzzer TARGET SECONDS ARTIFACT_DIRECTORY");
        process::exit(2);
    }
    let target = &arguments[1];
    let seconds = arguments[2].parse::<u64>().unwrap_or(0);
    if seconds == 0 || seconds > 30 {
        eprintln!("invalid bounded fuzz duration");
        process::exit(2);
    }
    let artifact_directory = PathBuf::from(&arguments[3]);
    fs::create_dir_all(&artifact_directory).expect("create artifact directory");
    let current = artifact_directory.join(format!("current-{target}.bin"));
    let seed = corpus(target).unwrap_or_else(|| {
        eprintln!("unknown fuzz target: {target}");
        process::exit(2);
    });
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut state = 0x6a09_e667_f3bc_c909u64 ^ target_hash(target.as_bytes());
    let mut iteration = 0u64;
    while Instant::now() < deadline {
        let input = mutate(seed, &mut state, iteration);
        fs::write(&current, &input).expect("persist current fuzz input");
        if catch_unwind(AssertUnwindSafe(|| run_target(target, &input))).is_err() {
            let crash =
                artifact_directory.join(format!("crash-{target}-{:016x}-{iteration}.bin", state));
            let _ = fs::rename(&current, &crash);
            eprintln!("fuzz panic: target={target} artifact={}", crash.display());
            process::exit(1);
        }
        iteration = iteration.wrapping_add(1);
    }
    fs::remove_file(&current).expect("remove completed fuzz input");
    println!(
        "SECURITY_FUZZ: target={target} seconds={seconds} cases={iteration} seed=0x{:016x} result=PASS",
        0x6a09_e667_f3bc_c909u64 ^ target_hash(target.as_bytes())
    );
}

fn corpus(target: &str) -> Option<&'static [u8]> {
    match target {
        "config" => Some(CONFIG_SEED),
        "selector" => Some(SELECTOR_SEED),
        "elf" => Some(ELF_SEED),
        "vfs-index" => Some(VFS_SEED),
        "kernel-path" => Some(PATH_SEED),
        "device" => Some(DEVICE_SEED),
        "syscall" | "audit" => Some(&[0; 32]),
        _ => None,
    }
}

fn target_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x1000_0000_01b3)
    })
}

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn mutate(seed: &[u8], state: &mut u64, iteration: u64) -> Vec<u8> {
    if iteration == 0 {
        return seed.to_vec();
    }
    let mut length = if iteration.is_multiple_of(97) {
        MAX_INPUT
    } else {
        (next(state) as usize) % (seed.len().saturating_mul(4).max(65))
    };
    length = length.min(MAX_INPUT);
    let mut output = vec![0u8; length];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = if index < seed.len() {
            seed[index]
        } else {
            next(state) as u8
        };
    }
    let mutations = 1 + (next(state) as usize % 16);
    for _ in 0..mutations {
        if output.is_empty() {
            break;
        }
        let index = next(state) as usize % output.len();
        output[index] ^= 1u8 << (next(state) as u8 & 7);
    }
    output
}

fn run_target(target: &str, input: &[u8]) {
    match target {
        "config" => {
            if let Ok(config) = InstanceConfig::parse(input) {
                let _ = config.normalized();
                let _ = config.effective_contract();
            }
        }
        "selector" => {
            let _ = validate_selector(input);
            let _ = selector_matches(input, b"syscall.openat");
            let _ = validate_resource_selector(input);
            let _ = resource_selector_matches(input, b"filesystem.root.write");
        }
        "elf" => {
            let mut address_space = AddressSpace::new();
            let _ = address_space.load_elf(input);
        }
        "vfs-index" => {
            if let Ok(index) = Index::new(input) {
                for offset in 0..=index.len() {
                    let _ = index.entry(offset);
                }
                let _ = index.resolve(b"/etc/os-release");
            }
        }
        "kernel-path" => {
            let sysfs = SysFs::new(&[] as &[DeviceView], 2);
            let mut output = [0u8; 512];
            let _ = sysfs.read(input, &mut output);
        }
        "device" => fuzz_device(input),
        "syscall" => {
            let values = words(input);
            let syscall = values[0];
            let _ = decide(syscall);
            let _ = decide_systemd(syscall, values[1] & 1 != 0);
            let _ = decide_balanced(syscall);
            let _ = balanced_surface(syscall);
            let _ = balanced_arguments_allowed(syscall, values[1], values[2], values[3]);
        }
        "audit" => {
            let values = words(input);
            let mut budget = AuditBudget::new(values[0].min(8192));
            for _ in 0..(values[1] as usize % 16_384) {
                let _ = budget.admit();
            }
            let _ = (budget.emitted(), budget.dropped());
        }
        _ => unreachable!(),
    }
}

fn words(input: &[u8]) -> [u64; 4] {
    let mut output = [0u64; 4];
    for (index, byte) in input.iter().take(32).enumerate() {
        output[index / 8] |= u64::from(*byte) << ((index % 8) * 8);
    }
    output
}

fn fuzz_device(input: &[u8]) {
    let _ = DeviceName::new(input);
    let values = words(input);
    let backend = MemoryBlock([0u8; 256]);
    if let Ok(mut partition) = Partition::new(backend, values[0], values[1], values[2] & 1 != 0) {
        let mut output = [0u8; 32];
        let _ = partition.read_at(values[3], &mut output);
        let _ = partition.write_at(values[3], &output);
        let _ = partition.close();
    }
}

struct MemoryBlock([u8; 256]);

impl BlockBackend for MemoryBlock {
    fn len(&self) -> u64 {
        self.0.len() as u64
    }

    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), DeviceError> {
        let start = usize::try_from(offset).map_err(|_| DeviceError::OutOfRange)?;
        let end = start
            .checked_add(output.len())
            .filter(|end| *end <= self.0.len())
            .ok_or(DeviceError::OutOfRange)?;
        output.copy_from_slice(&self.0[start..end]);
        Ok(())
    }

    fn write_at(&mut self, offset: u64, input: &[u8]) -> Result<(), DeviceError> {
        let start = usize::try_from(offset).map_err(|_| DeviceError::OutOfRange)?;
        let end = start
            .checked_add(input.len())
            .filter(|end| *end <= self.0.len())
            .ok_or(DeviceError::OutOfRange)?;
        self.0[start..end].copy_from_slice(input);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), DeviceError> {
        Ok(())
    }

    fn close(&mut self) -> Result<(), DeviceError> {
        Ok(())
    }
}

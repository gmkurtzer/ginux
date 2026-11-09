#![no_std]

use core::fmt;

pub const SCHEMA_VERSION: u64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Text<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> Text<N> {
    pub const fn empty() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ConfigError> {
        let mut text = Self::empty();
        for byte in bytes {
            text.push(*byte)?;
        }
        Ok(text)
    }

    fn push(&mut self, byte: u8) -> Result<(), ConfigError> {
        if self.len == N {
            return Err(ConfigError::ValueTooLong);
        }
        self.bytes[self.len] = byte;
        self.len += 1;
        Ok(())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextList<const ITEMS: usize, const N: usize> {
    values: [Text<N>; ITEMS],
    len: usize,
}

impl<const ITEMS: usize, const N: usize> TextList<ITEMS, N> {
    pub const fn empty() -> Self {
        Self {
            values: [Text::empty(); ITEMS],
            len: 0,
        }
    }

    fn push(&mut self, value: Text<N>) -> Result<(), ConfigError> {
        if self.len == ITEMS {
            return Err(ConfigError::TooManyItems);
        }
        self.values[self.len] = value;
        self.len += 1;
        Ok(())
    }

    pub fn as_slice(&self) -> &[Text<N>] {
        &self.values[..self.len]
    }
}

fn push_unique<const ITEMS: usize, const N: usize>(
    values: &mut TextList<ITEMS, N>,
    value: &[u8],
) -> Result<(), ConfigError> {
    if values
        .as_slice()
        .iter()
        .any(|existing| existing.as_bytes() == value)
    {
        return Ok(());
    }
    values.push(Text::from_bytes(value)?)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionProfile {
    Security,
    Balanced,
    Performance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionBackend {
    Interpreter,
    PtraceSysemu,
    PtraceNative,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchMode {
    Paranoid,
    Selective,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkBackend {
    None,
    Mediated,
    Shared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkProtocol {
    Tcp,
    Udp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditMode {
    Stderr,
    Disabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailurePolicy {
    Stop,
    Fail,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookFailurePolicy {
    Stop,
    Continue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractDefault {
    Deny,
}

// Field capacities of `InstanceConfig`, shared with `NORMALIZED_CAPACITY`.
const IDENTIFIER_BYTES: usize = 64;
const REFERENCE_BYTES: usize = 128;
const PATH_BYTES: usize = 128;
const ARG_ITEMS: usize = 7;
const ARG_BYTES: usize = 128;
const ENVIRONMENT_ITEMS: usize = 8;
const ENVIRONMENT_BYTES: usize = 256;
const ASSIGNMENT_ITEMS: usize = 8;
const ASSIGNMENT_BYTES: usize = 64;
const POLICY_ITEMS: usize = 16;
const POLICY_BYTES: usize = 128;
const CONTRACT_RULE_ITEMS: usize = 16;
const CONTRACT_RULE_BYTES: usize = 96;
const FILESYSTEM_RULE_ITEMS: usize = 16;
const DEVICE_RULE_ITEMS: usize = 16;
const PROCESS_RULE_ITEMS: usize = 16;
const KERNEL_RULE_ITEMS: usize = 16;
const HOOK_ITEMS: usize = 12;
const HOOK_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceConfig {
    pub schema_version: u64,
    pub id: Text<IDENTIFIER_BYTES>,
    pub hostname: Text<IDENTIFIER_BYTES>,
    pub root: Text<REFERENCE_BYTES>,
    pub init: Text<PATH_BYTES>,
    pub args: TextList<ARG_ITEMS, ARG_BYTES>,
    pub environment: TextList<ENVIRONMENT_ITEMS, ENVIRONMENT_BYTES>,
    pub workdir: Text<PATH_BYTES>,
    pub vcpus: u64,
    pub memory_mib: u64,
    pub execution_profile: ExecutionProfile,
    pub execution_backend: ExecutionBackend,
    pub dispatch_mode: DispatchMode,
    pub contract_profile: Text<IDENTIFIER_BYTES>,
    pub contract_templates: TextList<ASSIGNMENT_ITEMS, ASSIGNMENT_BYTES>,
    pub contract_allow: TextList<POLICY_ITEMS, POLICY_BYTES>,
    pub contract_deny: TextList<POLICY_ITEMS, POLICY_BYTES>,
    pub contract_recipe_version: u64,
    pub contract_catalog_version: u64,
    pub contract_default: ContractDefault,
    pub contract_read: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    pub contract_write: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    pub contract_execute: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    pub contract_connect: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    pub contract_listen: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    pub contract_process: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    pub contract_system: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    pub contract_recipe_deny: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    pub contract_exact_allow: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    pub contract_exact_deny: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    pub contract_digest: Text<REFERENCE_BYTES>,
    pub filesystem_read: TextList<FILESYSTEM_RULE_ITEMS, PATH_BYTES>,
    pub filesystem_write: TextList<FILESYSTEM_RULE_ITEMS, PATH_BYTES>,
    pub filesystem_create: TextList<FILESYSTEM_RULE_ITEMS, PATH_BYTES>,
    pub filesystem_write_deny: TextList<FILESYSTEM_RULE_ITEMS, PATH_BYTES>,
    pub privilege_setuid: TextList<FILESYSTEM_RULE_ITEMS, PATH_BYTES>,
    pub privilege_setgid: TextList<FILESYSTEM_RULE_ITEMS, PATH_BYTES>,
    pub filesystem_device_nodes: bool,
    pub device_read: TextList<DEVICE_RULE_ITEMS, PATH_BYTES>,
    pub device_write: TextList<DEVICE_RULE_ITEMS, PATH_BYTES>,
    pub device_create: TextList<DEVICE_RULE_ITEMS, PATH_BYTES>,
    pub process_max_processes: u64,
    pub process_max_threads: u64,
    pub process_signals: TextList<PROCESS_RULE_ITEMS, IDENTIFIER_BYTES>,
    pub process_ptrace: bool,
    pub process_namespace_create: TextList<PROCESS_RULE_ITEMS, IDENTIFIER_BYTES>,
    pub process_namespace_join: TextList<PROCESS_RULE_ITEMS, IDENTIFIER_BYTES>,
    pub kernel_proc_read: TextList<KERNEL_RULE_ITEMS, PATH_BYTES>,
    pub kernel_proc_write: TextList<KERNEL_RULE_ITEMS, PATH_BYTES>,
    pub kernel_sys_read: TextList<KERNEL_RULE_ITEMS, PATH_BYTES>,
    pub kernel_sys_write: TextList<KERNEL_RULE_ITEMS, PATH_BYTES>,
    pub storage: TextList<ASSIGNMENT_ITEMS, ASSIGNMENT_BYTES>,
    pub volumes: TextList<ASSIGNMENT_ITEMS, POLICY_BYTES>,
    pub devices: TextList<ASSIGNMENT_ITEMS, ASSIGNMENT_BYTES>,
    pub network_backend: NetworkBackend,
    pub network_interface: Text<IDENTIFIER_BYTES>,
    pub network_source: Text<IDENTIFIER_BYTES>,
    pub ingress_allow: TextList<POLICY_ITEMS, POLICY_BYTES>,
    pub ingress_deny: TextList<POLICY_ITEMS, POLICY_BYTES>,
    pub egress_allow: TextList<POLICY_ITEMS, POLICY_BYTES>,
    pub egress_deny: TextList<POLICY_ITEMS, POLICY_BYTES>,
    pub port_forwards: TextList<ASSIGNMENT_ITEMS, POLICY_BYTES>,
    pub access_uid: u64,
    pub access_gid: u64,
    pub pre_boot_hooks: TextList<HOOK_ITEMS, HOOK_BYTES>,
    pub hook_timeout_seconds: u64,
    pub hook_failure_policy: HookFailurePolicy,
    pub stop_timeout_seconds: u64,
    pub audit_mode: AuditMode,
    pub failure_policy: FailurePolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    InvalidUtf8,
    InvalidSyntax,
    UnknownSection,
    UnknownField,
    DuplicateField,
    MissingField,
    UnsupportedSchema,
    InvalidValue,
    ValueTooLong,
    TooManyItems,
    UnsupportedExecution,
    UnsupportedBackend,
    HostAuthority,
    InvalidContract,
    ContractDigestMismatch,
    DerivedField,
}

impl ConfigError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidUtf8 => "invalid-utf8",
            Self::InvalidSyntax => "invalid-toml-syntax",
            Self::UnknownSection => "unknown-section",
            Self::UnknownField => "unknown-field",
            Self::DuplicateField => "duplicate-field",
            Self::MissingField => "missing-field",
            Self::UnsupportedSchema => "unsupported-schema-version",
            Self::InvalidValue => "invalid-value",
            Self::ValueTooLong => "value-too-long",
            Self::TooManyItems => "too-many-items",
            Self::UnsupportedExecution => "unsupported-execution-profile",
            Self::UnsupportedBackend => "unsupported-backend",
            Self::HostAuthority => "host-authority-reference",
            Self::InvalidContract => "invalid-contract",
            Self::ContractDigestMismatch => "contract-digest-mismatch",
            Self::DerivedField => "derived-field",
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy)]
enum Section {
    Root,
    Instance,
    Identity,
    Resources,
    Execution,
    Contract,
    ContractSyscalls,
    ContractFilesystem,
    ContractNetwork,
    ContractDevices,
    ContractPrivileges,
    ContractProcess,
    ContractKernel,
    Storage,
    Volumes,
    Devices,
    Network,
    Access,
    Hooks,
    Lifecycle,
    Audit,
    Failure,
}

struct ConfigBuilder {
    seen: u128,
    schema_version: u64,
    id: Text<IDENTIFIER_BYTES>,
    hostname: Text<IDENTIFIER_BYTES>,
    root: Text<REFERENCE_BYTES>,
    init: Text<PATH_BYTES>,
    args: TextList<ARG_ITEMS, ARG_BYTES>,
    environment: TextList<ENVIRONMENT_ITEMS, ENVIRONMENT_BYTES>,
    workdir: Text<PATH_BYTES>,
    vcpus: u64,
    memory_mib: u64,
    execution_profile: ExecutionProfile,
    execution_backend: ExecutionBackend,
    dispatch_mode: DispatchMode,
    contract_profile: Text<IDENTIFIER_BYTES>,
    contract_templates: TextList<ASSIGNMENT_ITEMS, ASSIGNMENT_BYTES>,
    contract_allow: TextList<POLICY_ITEMS, POLICY_BYTES>,
    contract_deny: TextList<POLICY_ITEMS, POLICY_BYTES>,
    contract_recipe_version: u64,
    contract_catalog_version: u64,
    contract_default: ContractDefault,
    contract_read: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    contract_write: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    contract_execute: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    contract_connect: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    contract_listen: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    contract_process: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    contract_system: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    contract_recipe_deny: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    contract_exact_allow: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    contract_exact_deny: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>,
    contract_digest: Text<REFERENCE_BYTES>,
    filesystem_read: TextList<FILESYSTEM_RULE_ITEMS, PATH_BYTES>,
    filesystem_write: TextList<FILESYSTEM_RULE_ITEMS, PATH_BYTES>,
    filesystem_create: TextList<FILESYSTEM_RULE_ITEMS, PATH_BYTES>,
    filesystem_write_deny: TextList<FILESYSTEM_RULE_ITEMS, PATH_BYTES>,
    privilege_setuid: TextList<FILESYSTEM_RULE_ITEMS, PATH_BYTES>,
    privilege_setgid: TextList<FILESYSTEM_RULE_ITEMS, PATH_BYTES>,
    filesystem_device_nodes: bool,
    device_read: TextList<DEVICE_RULE_ITEMS, PATH_BYTES>,
    device_write: TextList<DEVICE_RULE_ITEMS, PATH_BYTES>,
    device_create: TextList<DEVICE_RULE_ITEMS, PATH_BYTES>,
    process_max_processes: u64,
    process_max_threads: u64,
    process_signals: TextList<PROCESS_RULE_ITEMS, IDENTIFIER_BYTES>,
    process_ptrace: bool,
    process_namespace_create: TextList<PROCESS_RULE_ITEMS, IDENTIFIER_BYTES>,
    process_namespace_join: TextList<PROCESS_RULE_ITEMS, IDENTIFIER_BYTES>,
    kernel_proc_read: TextList<KERNEL_RULE_ITEMS, PATH_BYTES>,
    kernel_proc_write: TextList<KERNEL_RULE_ITEMS, PATH_BYTES>,
    kernel_sys_read: TextList<KERNEL_RULE_ITEMS, PATH_BYTES>,
    kernel_sys_write: TextList<KERNEL_RULE_ITEMS, PATH_BYTES>,
    storage: TextList<ASSIGNMENT_ITEMS, ASSIGNMENT_BYTES>,
    volumes: TextList<ASSIGNMENT_ITEMS, POLICY_BYTES>,
    devices: TextList<ASSIGNMENT_ITEMS, ASSIGNMENT_BYTES>,
    network_backend: NetworkBackend,
    network_interface: Text<IDENTIFIER_BYTES>,
    network_source: Text<IDENTIFIER_BYTES>,
    ingress_allow: TextList<POLICY_ITEMS, POLICY_BYTES>,
    ingress_deny: TextList<POLICY_ITEMS, POLICY_BYTES>,
    egress_allow: TextList<POLICY_ITEMS, POLICY_BYTES>,
    egress_deny: TextList<POLICY_ITEMS, POLICY_BYTES>,
    port_forwards: TextList<ASSIGNMENT_ITEMS, POLICY_BYTES>,
    access_uid: u64,
    access_gid: u64,
    pre_boot_hooks: TextList<HOOK_ITEMS, HOOK_BYTES>,
    hook_timeout_seconds: u64,
    hook_failure_policy: HookFailurePolicy,
    stop_timeout_seconds: u64,
    audit_mode: AuditMode,
    failure_policy: FailurePolicy,
}

impl ConfigBuilder {
    const fn new() -> Self {
        Self {
            seen: 0,
            schema_version: 0,
            id: Text::empty(),
            hostname: Text::empty(),
            root: Text::empty(),
            init: Text::empty(),
            args: TextList::empty(),
            environment: TextList::empty(),
            workdir: Text::empty(),
            vcpus: 0,
            memory_mib: 0,
            execution_profile: ExecutionProfile::Security,
            execution_backend: ExecutionBackend::Interpreter,
            dispatch_mode: DispatchMode::Selective,
            contract_profile: Text::empty(),
            contract_templates: TextList::empty(),
            contract_allow: TextList::empty(),
            contract_deny: TextList::empty(),
            contract_recipe_version: 0,
            contract_catalog_version: 0,
            contract_default: ContractDefault::Deny,
            contract_read: TextList::empty(),
            contract_write: TextList::empty(),
            contract_execute: TextList::empty(),
            contract_connect: TextList::empty(),
            contract_listen: TextList::empty(),
            contract_process: TextList::empty(),
            contract_system: TextList::empty(),
            contract_recipe_deny: TextList::empty(),
            contract_exact_allow: TextList::empty(),
            contract_exact_deny: TextList::empty(),
            contract_digest: Text::empty(),
            filesystem_read: TextList::empty(),
            filesystem_write: TextList::empty(),
            filesystem_create: TextList::empty(),
            filesystem_write_deny: TextList::empty(),
            privilege_setuid: TextList::empty(),
            privilege_setgid: TextList::empty(),
            filesystem_device_nodes: false,
            device_read: TextList::empty(),
            device_write: TextList::empty(),
            device_create: TextList::empty(),
            process_max_processes: 4096,
            process_max_threads: 4096,
            process_signals: TextList::empty(),
            process_ptrace: false,
            process_namespace_create: TextList::empty(),
            process_namespace_join: TextList::empty(),
            kernel_proc_read: TextList::empty(),
            kernel_proc_write: TextList::empty(),
            kernel_sys_read: TextList::empty(),
            kernel_sys_write: TextList::empty(),
            storage: TextList::empty(),
            volumes: TextList::empty(),
            devices: TextList::empty(),
            network_backend: NetworkBackend::Shared,
            network_interface: Text::empty(),
            network_source: Text::empty(),
            ingress_allow: TextList::empty(),
            ingress_deny: TextList::empty(),
            egress_allow: TextList::empty(),
            egress_deny: TextList::empty(),
            port_forwards: TextList::empty(),
            access_uid: 0,
            access_gid: 0,
            pre_boot_hooks: TextList::empty(),
            hook_timeout_seconds: 30,
            hook_failure_policy: HookFailurePolicy::Stop,
            stop_timeout_seconds: 30,
            audit_mode: AuditMode::Stderr,
            failure_policy: FailurePolicy::Stop,
        }
    }

    fn mark(&mut self, bit: u128) -> Result<(), ConfigError> {
        if self.seen & bit != 0 {
            return Err(ConfigError::DuplicateField);
        }
        self.seen |= bit;
        Ok(())
    }

    fn finish(self) -> Result<InstanceConfig, ConfigError> {
        const REQUIRED: u128 = ((1 << 16) - 1) & !(1 << 13);
        const WORKDIR_BIT: u128 = 1 << 16;
        const HOSTNAME_BIT: u128 = 1 << 17;
        const CONTRACT_DIGEST_BIT: u128 = 1 << 21;
        const RECIPE_VERSION_BIT: u128 = 1 << 36;
        const RECIPE_CATALOG_BIT: u128 = 1 << 37;
        const RECIPE_DEFAULT_BIT: u128 = 1 << 38;
        if self.seen & REQUIRED != REQUIRED {
            return Err(ConfigError::MissingField);
        }
        if self.seen & RECIPE_VERSION_BIT != 0
            && self.seen & (RECIPE_CATALOG_BIT | RECIPE_DEFAULT_BIT)
                != RECIPE_CATALOG_BIT | RECIPE_DEFAULT_BIT
        {
            return Err(ConfigError::MissingField);
        }
        let workdir = if self.seen & WORKDIR_BIT != 0 {
            self.workdir
        } else {
            Text::from_bytes(b"/work").map_err(|_| ConfigError::InvalidValue)?
        };
        let hostname = if self.seen & HOSTNAME_BIT != 0 {
            self.hostname
        } else {
            self.id
        };
        let supplied_contract_digest = self.contract_digest;
        let supplied_contract_digest_set = self.seen & CONTRACT_DIGEST_BIT != 0;
        let mut config = InstanceConfig {
            schema_version: self.schema_version,
            id: self.id,
            hostname,
            root: self.root,
            init: self.init,
            args: self.args,
            environment: self.environment,
            workdir,
            vcpus: self.vcpus,
            memory_mib: self.memory_mib,
            execution_profile: self.execution_profile,
            execution_backend: self.execution_backend,
            dispatch_mode: self.dispatch_mode,
            contract_profile: self.contract_profile,
            contract_templates: self.contract_templates,
            contract_allow: self.contract_allow,
            contract_deny: self.contract_deny,
            contract_recipe_version: self.contract_recipe_version,
            contract_catalog_version: self.contract_catalog_version,
            contract_default: self.contract_default,
            contract_read: self.contract_read,
            contract_write: self.contract_write,
            contract_execute: self.contract_execute,
            contract_connect: self.contract_connect,
            contract_listen: self.contract_listen,
            contract_process: self.contract_process,
            contract_system: self.contract_system,
            contract_recipe_deny: self.contract_recipe_deny,
            contract_exact_allow: self.contract_exact_allow,
            contract_exact_deny: self.contract_exact_deny,
            contract_digest: Text::empty(),
            filesystem_read: self.filesystem_read,
            filesystem_write: self.filesystem_write,
            filesystem_create: self.filesystem_create,
            filesystem_write_deny: self.filesystem_write_deny,
            privilege_setuid: self.privilege_setuid,
            privilege_setgid: self.privilege_setgid,
            filesystem_device_nodes: self.filesystem_device_nodes,
            device_read: self.device_read,
            device_write: self.device_write,
            device_create: self.device_create,
            process_max_processes: self.process_max_processes,
            process_max_threads: self.process_max_threads,
            process_signals: self.process_signals,
            process_ptrace: self.process_ptrace,
            process_namespace_create: self.process_namespace_create,
            process_namespace_join: self.process_namespace_join,
            kernel_proc_read: self.kernel_proc_read,
            kernel_proc_write: self.kernel_proc_write,
            kernel_sys_read: self.kernel_sys_read,
            kernel_sys_write: self.kernel_sys_write,
            storage: self.storage,
            volumes: self.volumes,
            devices: self.devices,
            network_backend: self.network_backend,
            network_interface: self.network_interface,
            network_source: self.network_source,
            ingress_allow: self.ingress_allow,
            ingress_deny: self.ingress_deny,
            egress_allow: self.egress_allow,
            egress_deny: self.egress_deny,
            port_forwards: self.port_forwards,
            access_uid: self.access_uid,
            access_gid: self.access_gid,
            pre_boot_hooks: self.pre_boot_hooks,
            hook_timeout_seconds: self.hook_timeout_seconds,
            hook_failure_policy: self.hook_failure_policy,
            stop_timeout_seconds: self.stop_timeout_seconds,
            audit_mode: self.audit_mode,
            failure_policy: self.failure_policy,
        };
        config.derive_contract_grants()?;
        config.validate()?;
        config.refresh_contract()?;
        if supplied_contract_digest_set && supplied_contract_digest != config.contract_digest {
            return Err(ConfigError::ContractDigestMismatch);
        }
        Ok(config)
    }
}

impl InstanceConfig {
    fn derive_contract_grants(&mut self) -> Result<(), ConfigError> {
        if !self.filesystem_read.as_slice().is_empty() {
            push_unique(&mut self.contract_read, b"filesystem.**")?;
        }
        if !self.filesystem_write.as_slice().is_empty()
            || !self.filesystem_create.as_slice().is_empty()
        {
            push_unique(&mut self.contract_write, b"filesystem.**")?;
        }
        if !self.device_read.as_slice().is_empty() {
            push_unique(&mut self.contract_read, b"device.**")?;
        }
        if !self.device_write.as_slice().is_empty() || !self.device_create.as_slice().is_empty() {
            push_unique(&mut self.contract_write, b"device.**")?;
        }
        if !self.ingress_allow.as_slice().is_empty() {
            push_unique(&mut self.contract_listen, b"network.**")?;
        }
        if !self.egress_allow.as_slice().is_empty() {
            push_unique(&mut self.contract_connect, b"network.**")?;
        }
        if !self.process_signals.as_slice().is_empty() {
            push_unique(&mut self.contract_process, b"signal.**")?;
        }
        if !self.kernel_proc_read.as_slice().is_empty() {
            push_unique(&mut self.contract_read, b"proc.**")?;
        }
        if !self.kernel_proc_write.as_slice().is_empty() {
            push_unique(&mut self.contract_write, b"proc.**")?;
        }
        if !self.kernel_sys_read.as_slice().is_empty() {
            push_unique(&mut self.contract_read, b"sys.**")?;
        }
        if !self.kernel_sys_write.as_slice().is_empty() {
            push_unique(&mut self.contract_write, b"sys.**")?;
        }
        Ok(())
    }

    pub fn parse(input: &[u8]) -> Result<Self, ConfigError> {
        let source = core::str::from_utf8(input).map_err(|_| ConfigError::InvalidUtf8)?;
        let mut builder = ConfigBuilder::new();
        let mut section = Section::Root;
        for original_line in source.lines() {
            let line = strip_comment(original_line.as_bytes())?;
            let line = trim(line);
            if line.is_empty() {
                continue;
            }
            if line[0] == b'[' {
                section = parse_section(line)?;
                continue;
            }
            let equals = line
                .iter()
                .position(|byte| *byte == b'=')
                .ok_or(ConfigError::InvalidSyntax)?;
            let key = trim(&line[..equals]);
            let value = trim(&line[equals + 1..]);
            if key.is_empty() || value.is_empty() || !is_key(key) {
                return Err(ConfigError::InvalidSyntax);
            }
            parse_field(&mut builder, section, key, value)?;
        }
        builder.finish()
    }

    /// Apply one command-line override using the same TOML value parsers and
    /// validation rules as file configuration. `key` is a fully-qualified
    /// schema path such as `resources.vcpus`; `value` is the TOML value only.
    pub fn apply_override(&mut self, key: &[u8], value: &[u8]) -> Result<(), ConfigError> {
        let mut candidate = *self;
        candidate.apply_override_inner(key, value)?;
        candidate.contract_read = TextList::empty();
        candidate.contract_write = TextList::empty();
        candidate.contract_connect = TextList::empty();
        candidate.contract_listen = TextList::empty();
        candidate.contract_process = TextList::empty();
        candidate.derive_contract_grants()?;
        candidate.validate()?;
        candidate.refresh_contract()?;
        *self = candidate;
        Ok(())
    }

    fn apply_override_inner(&mut self, key: &[u8], value: &[u8]) -> Result<(), ConfigError> {
        match key {
            b"instance.id" => self.id = parse_string(value)?,
            b"instance.root" => self.root = parse_string(value)?,
            b"instance.init" => self.init = parse_string(value)?,
            b"instance.args" => self.args = parse_list(value)?,
            b"instance.environment" => self.environment = parse_list(value)?,
            b"instance.workdir" => self.workdir = parse_string(value)?,
            b"identity.hostname" => self.hostname = parse_string(value)?,
            b"resources.vcpus" => self.vcpus = parse_u64(value)?,
            b"resources.memory_mib" => self.memory_mib = parse_u64(value)?,
            b"execution.profile" => {
                self.execution_profile = match parse_string::<32>(value)?.as_bytes() {
                    b"security" => ExecutionProfile::Security,
                    b"balanced" => ExecutionProfile::Balanced,
                    b"performance" => ExecutionProfile::Performance,
                    _ => return Err(ConfigError::InvalidValue),
                }
            }
            b"execution.backend" => {
                self.execution_backend = match parse_string::<32>(value)?.as_bytes() {
                    b"interpreter" => ExecutionBackend::Interpreter,
                    b"ptrace-sysemu" => ExecutionBackend::PtraceSysemu,
                    b"ptrace-native" => ExecutionBackend::PtraceNative,
                    _ => return Err(ConfigError::InvalidValue),
                }
            }
            b"contract.syscalls.control" => {
                self.dispatch_mode = match parse_string::<32>(value)?.as_bytes() {
                    b"paranoid" => DispatchMode::Paranoid,
                    b"selective" => DispatchMode::Selective,
                    _ => return Err(ConfigError::InvalidValue),
                }
            }
            b"contract.version" => self.contract_recipe_version = parse_u64(value)?,
            b"contract.catalog" => self.contract_catalog_version = parse_u64(value)?,
            b"contract.default" => self.contract_default = parse_contract_default(value)?,
            b"contract.allow" => self.contract_allow = parse_list(value)?,
            b"contract.deny" => self.contract_deny = parse_list(value)?,
            b"contract.digest" => return Err(ConfigError::DerivedField),
            b"contract.syscalls.default" => {
                parse_contract_default(value)?;
            }
            b"contract.syscalls.allow" => self.contract_exact_allow = parse_syscall_list(value)?,
            b"contract.syscalls.deny" => self.contract_exact_deny = parse_syscall_list(value)?,
            b"contract.filesystem.default" => {
                parse_contract_default(value)?;
            }
            b"contract.filesystem.read" => self.filesystem_read = parse_list(value)?,
            b"contract.filesystem.write" => self.filesystem_write = parse_list(value)?,
            b"contract.filesystem.create" => self.filesystem_create = parse_list(value)?,
            b"contract.filesystem.deny" => self.filesystem_write_deny = parse_list(value)?,
            b"contract.network.default" => {
                parse_contract_default(value)?;
            }
            b"contract.network.inbound" => self.ingress_allow = parse_list(value)?,
            b"contract.network.outbound" => self.egress_allow = parse_list(value)?,
            b"contract.devices.default" => {
                parse_contract_default(value)?;
            }
            b"contract.devices.read" => self.device_read = parse_list(value)?,
            b"contract.devices.write" => self.device_write = parse_list(value)?,
            b"contract.devices.create" => {
                self.device_create = parse_list(value)?;
                self.filesystem_device_nodes = !self.device_create.as_slice().is_empty();
            }
            b"contract.privileges.default" => {
                parse_contract_default(value)?;
            }
            b"contract.privileges.setuid" => self.privilege_setuid = parse_list(value)?,
            b"contract.privileges.setgid" => self.privilege_setgid = parse_list(value)?,
            b"contract.process.default" => {
                parse_contract_default(value)?;
            }
            b"contract.process.max_processes" => self.process_max_processes = parse_u64(value)?,
            b"contract.process.max_threads" => self.process_max_threads = parse_u64(value)?,
            b"contract.process.signals" => self.process_signals = parse_list(value)?,
            b"contract.process.ptrace" => self.process_ptrace = parse_allow_deny(value)?,
            b"contract.process.namespace_create" => {
                self.process_namespace_create = parse_list(value)?
            }
            b"contract.process.namespace_join" => self.process_namespace_join = parse_list(value)?,
            b"contract.kernel.default" => {
                parse_contract_default(value)?;
            }
            b"contract.kernel.proc_read" => self.kernel_proc_read = parse_list(value)?,
            b"contract.kernel.proc_write" => self.kernel_proc_write = parse_list(value)?,
            b"contract.kernel.sys_read" => self.kernel_sys_read = parse_list(value)?,
            b"contract.kernel.sys_write" => self.kernel_sys_write = parse_list(value)?,
            b"storage.assignments" => self.storage = parse_list(value)?,
            b"volumes.assignments" => self.volumes = parse_list(value)?,
            b"devices.assignments" => self.devices = parse_list(value)?,
            b"network.backend" => {
                self.network_backend = match parse_string::<32>(value)?.as_bytes() {
                    b"none" => NetworkBackend::None,
                    b"mediated" => NetworkBackend::Mediated,
                    b"shared" => NetworkBackend::Shared,
                    _ => return Err(ConfigError::InvalidValue),
                }
            }
            b"network.interface" => self.network_interface = parse_string(value)?,
            b"network.source" => self.network_source = parse_string(value)?,
            b"network.ingress_allow" => self.ingress_allow = parse_list(value)?,
            b"network.ingress_deny" => self.ingress_deny = parse_list(value)?,
            b"network.egress_allow" => self.egress_allow = parse_list(value)?,
            b"network.egress_deny" => self.egress_deny = parse_list(value)?,
            b"network.port_forwards" => self.port_forwards = parse_list(value)?,
            b"access.uid" => self.access_uid = parse_u64(value)?,
            b"access.gid" => self.access_gid = parse_u64(value)?,
            b"hooks.pre_boot" => self.pre_boot_hooks = parse_list(value)?,
            b"hooks.timeout_seconds" => self.hook_timeout_seconds = parse_u64(value)?,
            b"hooks.failure" => {
                self.hook_failure_policy = match parse_string::<32>(value)?.as_bytes() {
                    b"stop" => HookFailurePolicy::Stop,
                    b"continue" => HookFailurePolicy::Continue,
                    _ => return Err(ConfigError::InvalidValue),
                }
            }
            b"lifecycle.stop_timeout_seconds" => self.stop_timeout_seconds = parse_u64(value)?,
            b"audit.mode" => {
                self.audit_mode = match parse_string::<32>(value)?.as_bytes() {
                    b"stderr" => AuditMode::Stderr,
                    b"disabled" => AuditMode::Disabled,
                    _ => return Err(ConfigError::InvalidValue),
                }
            }
            b"failure.policy" => {
                self.failure_policy = match parse_string::<32>(value)?.as_bytes() {
                    b"stop" => FailurePolicy::Stop,
                    b"fail" => FailurePolicy::Fail,
                    _ => return Err(ConfigError::InvalidValue),
                }
            }
            _ => return Err(ConfigError::UnknownField),
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedSchema);
        }
        validate_identifier(self.id.as_bytes())?;
        validate_reference(self.root.as_bytes())?;
        validate_instance_path(self.init.as_bytes())?;
        if self.workdir.as_bytes() != b"/" {
            validate_instance_path(self.workdir.as_bytes())?;
        }
        if self.contract_recipe_version != 1
            || self.contract_catalog_version != ginux_contract::CATALOG_VERSION as u64
            || !self.contract_profile.is_empty()
            || !self.contract_templates.as_slice().is_empty()
        {
            return Err(ConfigError::InvalidContract);
        }
        validate_identifier(self.hostname.as_bytes())?;
        if !self.contract_digest.is_empty() {
            validate_reference(self.contract_digest.as_bytes())?;
        }
        if !(1..=64).contains(&self.vcpus) || !(16..=1_048_576).contains(&self.memory_mib) {
            return Err(ConfigError::InvalidValue);
        }
        if self.access_uid > u32::MAX as u64
            || self.access_gid > u32::MAX as u64
            || self.hook_timeout_seconds == 0
            || self.hook_timeout_seconds > 3600
            || self.stop_timeout_seconds == 0
            || self.stop_timeout_seconds > 3600
        {
            return Err(ConfigError::InvalidValue);
        }
        let supported_execution = matches!(
            (self.execution_profile, self.execution_backend),
            (ExecutionProfile::Security, ExecutionBackend::Interpreter)
                | (ExecutionProfile::Balanced, ExecutionBackend::PtraceNative)
                | (
                    ExecutionProfile::Performance,
                    ExecutionBackend::PtraceNative
                )
        );
        if !supported_execution {
            return Err(ConfigError::UnsupportedExecution);
        }
        for value in self
            .storage
            .as_slice()
            .iter()
            .chain(self.devices.as_slice())
            .chain(self.contract_templates.as_slice())
        {
            validate_reference(value.as_bytes())?;
        }
        for selector in self
            .contract_allow
            .as_slice()
            .iter()
            .chain(self.contract_deny.as_slice())
        {
            ginux_contract::validate_selector(selector.as_bytes())
                .map_err(|_| ConfigError::InvalidContract)?;
        }
        for selector in self
            .contract_read
            .as_slice()
            .iter()
            .chain(self.contract_write.as_slice())
            .chain(self.contract_execute.as_slice())
            .chain(self.contract_connect.as_slice())
            .chain(self.contract_listen.as_slice())
            .chain(self.contract_process.as_slice())
            .chain(self.contract_system.as_slice())
            .chain(self.contract_recipe_deny.as_slice())
        {
            ginux_contract::validate_resource_selector(selector.as_bytes())
                .map_err(|_| ConfigError::InvalidContract)?;
        }
        for selector in self
            .contract_exact_allow
            .as_slice()
            .iter()
            .chain(self.contract_exact_deny.as_slice())
        {
            ginux_contract::validate_selector(selector.as_bytes())
                .map_err(|_| ConfigError::InvalidContract)?;
            if !selector.as_bytes().starts_with(b"syscall.") {
                return Err(ConfigError::InvalidContract);
            }
        }
        for (index, volume) in self.volumes.as_slice().iter().enumerate() {
            validate_volume(volume.as_bytes())?;
            let target = volume_target(volume.as_bytes()).ok_or(ConfigError::InvalidValue)?;
            if self.volumes.as_slice()[..index]
                .iter()
                .any(|existing| volume_target(existing.as_bytes()) == Some(target))
            {
                return Err(ConfigError::InvalidValue);
            }
        }
        if !self.network_interface.is_empty() {
            validate_network_attachment(self.network_interface.as_bytes())?;
        }
        if !self.network_source.is_empty() {
            validate_network_attachment(self.network_source.as_bytes())?;
            if self.network_source_ipv4().is_none() {
                return Err(ConfigError::InvalidValue);
            }
        }
        for rule in self
            .ingress_allow
            .as_slice()
            .iter()
            .chain(self.ingress_deny.as_slice())
            .chain(self.egress_allow.as_slice())
            .chain(self.egress_deny.as_slice())
        {
            validate_network_rule(rule.as_bytes())?;
        }
        for (index, forward) in self.port_forwards.as_slice().iter().enumerate() {
            validate_port_forward(forward.as_bytes())?;
            if self.port_forwards.as_slice()[..index]
                .iter()
                .any(|existing| port_forward_conflicts(existing.as_bytes(), forward.as_bytes()))
            {
                return Err(ConfigError::InvalidValue);
            }
        }
        for hook in self.pre_boot_hooks.as_slice() {
            validate_instance_command(hook.as_bytes())?;
        }
        if self.network_backend != NetworkBackend::Shared
            && (!self.network_interface.is_empty()
                || !self.network_source.is_empty()
                || !self.ingress_allow.as_slice().is_empty()
                || !self.ingress_deny.as_slice().is_empty()
                || !self.egress_allow.as_slice().is_empty()
                || !self.egress_deny.as_slice().is_empty()
                || !self.port_forwards.as_slice().is_empty())
        {
            return Err(ConfigError::InvalidValue);
        }
        for value in self.environment.as_slice() {
            let bytes = value.as_bytes();
            let equals = bytes.iter().position(|byte| *byte == b'=');
            if equals == Some(0) || equals.is_none() {
                return Err(ConfigError::InvalidValue);
            }
        }
        for path in self.filesystem_write_deny.as_slice() {
            validate_policy_path(path.as_bytes())?;
        }
        for path in self
            .filesystem_read
            .as_slice()
            .iter()
            .chain(self.filesystem_write.as_slice())
            .chain(self.filesystem_create.as_slice())
            .chain(self.privilege_setuid.as_slice())
            .chain(self.privilege_setgid.as_slice())
            .chain(self.device_read.as_slice())
            .chain(self.device_write.as_slice())
            .chain(self.device_create.as_slice())
            .chain(self.kernel_proc_read.as_slice())
            .chain(self.kernel_proc_write.as_slice())
            .chain(self.kernel_sys_read.as_slice())
            .chain(self.kernel_sys_write.as_slice())
        {
            validate_policy_path(path.as_bytes())?;
        }
        for device in self
            .process_signals
            .as_slice()
            .iter()
            .chain(self.process_namespace_create.as_slice())
            .chain(self.process_namespace_join.as_slice())
        {
            if device.as_bytes() != b"*" {
                validate_identifier(device.as_bytes())?;
            }
        }
        if self.process_max_processes == 0
            || self.process_max_threads == 0
            || self.process_max_threads < self.process_max_processes
        {
            return Err(ConfigError::InvalidValue);
        }
        Ok(())
    }

    pub fn effective_contract(&self) -> Result<ginux_contract::EffectiveContract, ConfigError> {
        if self.contract_recipe_version != 0 {
            let (allow, allow_len) = text_list_refs(&self.contract_allow);
            let (deny_all, deny_all_len) = text_list_refs(&self.contract_deny);
            let (read, read_len) = text_list_refs(&self.contract_read);
            let (write, write_len) = text_list_refs(&self.contract_write);
            let (execute, execute_len) = text_list_refs(&self.contract_execute);
            let (connect, connect_len) = text_list_refs(&self.contract_connect);
            let (listen, listen_len) = text_list_refs(&self.contract_listen);
            let (process, process_len) = text_list_refs(&self.contract_process);
            let (system, system_len) = text_list_refs(&self.contract_system);
            let (deny, deny_len) = text_list_refs(&self.contract_recipe_deny);
            let (exact_allow, exact_allow_len) = text_list_refs(&self.contract_exact_allow);
            let (exact_deny, exact_deny_len) = text_list_refs(&self.contract_exact_deny);
            return ginux_contract::resolve_recipe(ginux_contract::Recipe {
                allow: &allow[..allow_len],
                deny_all: &deny_all[..deny_all_len],
                read: &read[..read_len],
                write: &write[..write_len],
                execute: &execute[..execute_len],
                connect: &connect[..connect_len],
                listen: &listen[..listen_len],
                process: &process[..process_len],
                system: &system[..system_len],
                deny: &deny[..deny_len],
                syscall_allow: &exact_allow[..exact_allow_len],
                syscall_deny: &exact_deny[..exact_deny_len],
            })
            .map_err(|_| ConfigError::InvalidContract);
        }
        let mut templates = [&[][..]; ASSIGNMENT_ITEMS];
        for (index, value) in self.contract_templates.as_slice().iter().enumerate() {
            templates[index] = value.as_bytes();
        }
        let mut allow = [&[][..]; POLICY_ITEMS];
        for (index, value) in self.contract_allow.as_slice().iter().enumerate() {
            allow[index] = value.as_bytes();
        }
        let mut deny = [&[][..]; POLICY_ITEMS];
        for (index, value) in self.contract_deny.as_slice().iter().enumerate() {
            deny[index] = value.as_bytes();
        }
        ginux_contract::resolve(
            self.contract_profile.as_bytes(),
            &templates[..self.contract_templates.as_slice().len()],
            &allow[..self.contract_allow.as_slice().len()],
            &deny[..self.contract_deny.as_slice().len()],
        )
        .map_err(|_| ConfigError::InvalidContract)
    }

    fn refresh_contract(&mut self) -> Result<(), ConfigError> {
        let effective = self.effective_contract()?;
        self.contract_digest =
            Text::from_bytes(effective.digest()).map_err(|_| ConfigError::InvalidContract)?;
        Ok(())
    }

    pub fn normalized(&self) -> Result<NormalizedConfig, ConfigError> {
        let mut output = NormalizedConfig::new();
        output.raw(b"schema_version = ")?;
        output.number(self.schema_version)?;
        output.raw(b"\n\n[instance]\nid = ")?;
        output.quoted(self.id.as_bytes())?;
        output.raw(b"\nroot = ")?;
        output.quoted(self.root.as_bytes())?;
        output.raw(b"\ninit = ")?;
        output.quoted(self.init.as_bytes())?;
        output.raw(b"\nargs = ")?;
        output.list(self.args.as_slice())?;
        output.raw(b"\nenvironment = ")?;
        output.list(self.environment.as_slice())?;
        output.raw(b"\nworkdir = ")?;
        output.quoted(self.workdir.as_bytes())?;
        output.raw(b"\n\n[identity]\nhostname = ")?;
        output.quoted(self.hostname.as_bytes())?;
        output.raw(b"\n\n[resources]\nvcpus = ")?;
        output.number(self.vcpus)?;
        output.raw(b"\nmemory_mib = ")?;
        output.number(self.memory_mib)?;
        output.raw(b"\n\n[execution]\nprofile = ")?;
        output.quoted(match self.execution_profile {
            ExecutionProfile::Security => b"security",
            ExecutionProfile::Balanced => b"balanced",
            ExecutionProfile::Performance => b"performance",
        })?;
        output.raw(b"\nbackend = ")?;
        output.quoted(match self.execution_backend {
            ExecutionBackend::Interpreter => b"interpreter",
            ExecutionBackend::PtraceSysemu => b"ptrace-sysemu",
            ExecutionBackend::PtraceNative => b"ptrace-native",
        })?;
        output.raw(b"\n\n[contract]\nversion = ")?;
        output.number(self.contract_recipe_version)?;
        output.raw(b"\ncatalog = ")?;
        output.number(self.contract_catalog_version)?;
        output.raw(b"\ndefault = \"deny\"\nallow = ")?;
        output.list(self.contract_allow.as_slice())?;
        output.raw(b"\ndeny = ")?;
        output.list(self.contract_deny.as_slice())?;
        output.raw(b"\ndigest = ")?;
        output.quoted(self.contract_digest.as_bytes())?;
        output.raw(b"\n\n[contract.syscalls]\ncontrol = ")?;
        output.quoted(match self.dispatch_mode {
            DispatchMode::Paranoid => b"paranoid",
            DispatchMode::Selective => b"selective",
        })?;
        output.raw(b"\ndefault = \"deny\"\nallow = ")?;
        output.syscall_list(self.contract_exact_allow.as_slice())?;
        output.raw(b"\ndeny = ")?;
        output.syscall_list(self.contract_exact_deny.as_slice())?;
        output.raw(b"\n\n[contract.filesystem]\ndefault = \"deny\"\nread = ")?;
        output.list(self.filesystem_read.as_slice())?;
        output.raw(b"\nwrite = ")?;
        output.list(self.filesystem_write.as_slice())?;
        output.raw(b"\ncreate = ")?;
        output.list(self.filesystem_create.as_slice())?;
        output.raw(b"\ndeny = ")?;
        output.list(self.filesystem_write_deny.as_slice())?;
        output.raw(b"\n\n[contract.network]\ndefault = \"deny\"\ninbound = ")?;
        output.list(self.ingress_allow.as_slice())?;
        output.raw(b"\noutbound = ")?;
        output.list(self.egress_allow.as_slice())?;
        output.raw(b"\n\n[contract.devices]\ndefault = \"deny\"\nread = ")?;
        output.list(self.device_read.as_slice())?;
        output.raw(b"\nwrite = ")?;
        output.list(self.device_write.as_slice())?;
        output.raw(b"\ncreate = ")?;
        output.list(self.device_create.as_slice())?;
        output.raw(b"\n\n[contract.privileges]\ndefault = \"deny\"\nsetuid = ")?;
        output.list(self.privilege_setuid.as_slice())?;
        output.raw(b"\nsetgid = ")?;
        output.list(self.privilege_setgid.as_slice())?;
        output.raw(b"\n\n[contract.process]\ndefault = \"deny\"\nmax_processes = ")?;
        output.number(self.process_max_processes)?;
        output.raw(b"\nmax_threads = ")?;
        output.number(self.process_max_threads)?;
        output.raw(b"\nsignals = ")?;
        output.list(self.process_signals.as_slice())?;
        output.raw(b"\nptrace = ")?;
        output.quoted(if self.process_ptrace {
            b"allow"
        } else {
            b"deny"
        })?;
        output.raw(b"\nnamespace_create = ")?;
        output.list(self.process_namespace_create.as_slice())?;
        output.raw(b"\nnamespace_join = ")?;
        output.list(self.process_namespace_join.as_slice())?;
        output.raw(b"\n\n[contract.kernel]\ndefault = \"deny\"\nproc_read = ")?;
        output.list(self.kernel_proc_read.as_slice())?;
        output.raw(b"\nproc_write = ")?;
        output.list(self.kernel_proc_write.as_slice())?;
        output.raw(b"\nsys_read = ")?;
        output.list(self.kernel_sys_read.as_slice())?;
        output.raw(b"\nsys_write = ")?;
        output.list(self.kernel_sys_write.as_slice())?;
        output.raw(b"\n\n[storage]\nassignments = ")?;
        output.list(self.storage.as_slice())?;
        output.raw(b"\n\n[volumes]\nassignments = ")?;
        output.list(self.volumes.as_slice())?;
        output.raw(b"\n\n[devices]\nassignments = ")?;
        output.list(self.devices.as_slice())?;
        if self.network_backend != NetworkBackend::Shared
            || !self.network_interface.is_empty()
            || !self.network_source.is_empty()
            || !self.port_forwards.as_slice().is_empty()
        {
            output.raw(b"\n\n[network]\nbackend = ")?;
            output.quoted(match self.network_backend {
                NetworkBackend::None => b"none",
                NetworkBackend::Mediated => b"mediated",
                NetworkBackend::Shared => b"shared",
            })?;
            output.raw(b"\ninterface = ")?;
            output.quoted(self.network_interface.as_bytes())?;
            output.raw(b"\nsource = ")?;
            output.quoted(self.network_source.as_bytes())?;
            output.raw(b"\nport_forwards = ")?;
            output.list(self.port_forwards.as_slice())?;
        }
        output.raw(b"\n\n[access]\nuid = ")?;
        output.number(self.access_uid)?;
        output.raw(b"\ngid = ")?;
        output.number(self.access_gid)?;
        output.raw(b"\n\n[hooks]\npre_boot = ")?;
        output.list(self.pre_boot_hooks.as_slice())?;
        output.raw(b"\ntimeout_seconds = ")?;
        output.number(self.hook_timeout_seconds)?;
        output.raw(b"\nfailure = ")?;
        output.quoted(match self.hook_failure_policy {
            HookFailurePolicy::Stop => b"stop",
            HookFailurePolicy::Continue => b"continue",
        })?;
        output.raw(b"\n\n[lifecycle]\nstop_timeout_seconds = ")?;
        output.number(self.stop_timeout_seconds)?;
        output.raw(b"\n\n[audit]\nmode = ")?;
        output.quoted(match self.audit_mode {
            AuditMode::Stderr => b"stderr",
            AuditMode::Disabled => b"disabled",
        })?;
        output.raw(b"\n\n[failure]\npolicy = ")?;
        output.quoted(match self.failure_policy {
            FailurePolicy::Stop => b"stop",
            FailurePolicy::Fail => b"fail",
        })?;
        output.raw(b"\n")?;
        Ok(output)
    }

    pub fn network_allows(&self, ingress: bool, protocol: NetworkProtocol, port: u16) -> bool {
        let (allow, deny) = if ingress {
            (&self.ingress_allow, &self.ingress_deny)
        } else {
            (&self.egress_allow, &self.egress_deny)
        };
        if deny
            .as_slice()
            .iter()
            .any(|rule| network_rule_matches(rule.as_bytes(), protocol, port))
        {
            return false;
        }
        allow.as_slice().is_empty()
            || allow
                .as_slice()
                .iter()
                .any(|rule| network_rule_matches(rule.as_bytes(), protocol, port))
    }

    /// Returns true when `path` is the configured denied object or one of its
    /// descendants. Component boundaries prevent `/locked` from matching
    /// `/locked-other`.
    pub fn filesystem_write_denied(&self, path: &[u8]) -> bool {
        policy_path_matches(&self.filesystem_write_deny, path)
    }

    pub fn privilege_setid_denied(&self, path: &[u8], setuid: bool, setgid: bool) -> bool {
        (setuid && !policy_path_matches(&self.privilege_setuid, path))
            || (setgid && !policy_path_matches(&self.privilege_setgid, path))
    }

    pub fn device_create_denied(&self, path: &[u8]) -> bool {
        !policy_path_matches(&self.device_create, path)
    }

    pub fn translated_port(&self, protocol: NetworkProtocol, internal: u16) -> Option<u16> {
        for forward in self.port_forwards.as_slice() {
            let mut parts = forward.as_bytes().split(|byte| *byte == b'/');
            let forward_protocol = parts.next()?;
            let _attachment = parts.next()?;
            let external = parts.next()?;
            let configured_internal = parts.next()?;
            if parts.next().is_some() {
                continue;
            }
            let protocol_matches = match protocol {
                NetworkProtocol::Tcp => forward_protocol == b"tcp",
                NetworkProtocol::Udp => forward_protocol == b"udp",
            };
            if protocol_matches && parse_port(configured_internal) == Some(internal) {
                return parse_port(external);
            }
        }
        None
    }

    pub fn network_source_ipv4(&self) -> Option<[u8; 4]> {
        let bytes = self.network_source.as_bytes();
        if bytes.is_empty() {
            return None;
        }
        let mut output = [0u8; 4];
        let mut parts = bytes.split(|byte| *byte == b'.');
        for octet in &mut output {
            let part = parts.next()?;
            if part.is_empty() || part.len() > 3 || !part.iter().all(u8::is_ascii_digit) {
                return None;
            }
            let mut value = 0u16;
            for byte in part {
                value = value * 10 + u16::from(*byte - b'0');
            }
            if value > 255 {
                return None;
            }
            *octet = value as u8;
        }
        if parts.next().is_some() {
            return None;
        }
        Some(output)
    }
}

fn policy_path_matches<const ITEMS: usize>(
    rules: &TextList<ITEMS, PATH_BYTES>,
    path: &[u8],
) -> bool {
    rules.as_slice().iter().any(|rule| {
        let rule = rule.as_bytes();
        path == rule
            || rule == b"/"
            || (path.starts_with(rule) && path.get(rule.len()).is_some_and(|byte| *byte == b'/'))
    })
}

fn text_list_refs<const ITEMS: usize, const BYTES: usize>(
    values: &TextList<ITEMS, BYTES>,
) -> ([&[u8]; ITEMS], usize) {
    let mut references = [&[][..]; ITEMS];
    for (index, value) in values.as_slice().iter().enumerate() {
        references[index] = value.as_bytes();
    }
    (references, values.as_slice().len())
}

/// Text capacity of every `InstanceConfig` field: id, root, init, args,
/// environment, workdir, contract profile, storage and devices.
const CONFIG_TEXT_BYTES: usize = IDENTIFIER_BYTES
    + IDENTIFIER_BYTES
    + REFERENCE_BYTES
    + PATH_BYTES
    + ARG_ITEMS * ARG_BYTES
    + ENVIRONMENT_ITEMS * ENVIRONMENT_BYTES
    + PATH_BYTES
    + IDENTIFIER_BYTES
    + ASSIGNMENT_ITEMS * ASSIGNMENT_BYTES
    + 2 * POLICY_ITEMS * POLICY_BYTES
    + 10 * CONTRACT_RULE_ITEMS * CONTRACT_RULE_BYTES
    + REFERENCE_BYTES
    + FILESYSTEM_RULE_ITEMS * PATH_BYTES
    + 2 * ASSIGNMENT_ITEMS * ASSIGNMENT_BYTES
    + ASSIGNMENT_ITEMS * POLICY_BYTES
    + 2 * IDENTIFIER_BYTES
    + 4 * POLICY_ITEMS * POLICY_BYTES
    + ASSIGNMENT_ITEMS * POLICY_BYTES
    + HOOK_ITEMS * HOOK_BYTES;
/// Upper bound of `normalized()`: each text byte may be escaped to two
/// bytes; 1 KiB covers quotes, list separators, numbers, keys and enums
/// (about 500 bytes).
const NORMALIZED_CAPACITY: usize = 2 * CONFIG_TEXT_BYTES + 4096;

pub struct NormalizedConfig {
    bytes: [u8; NORMALIZED_CAPACITY],
    len: usize,
}

impl NormalizedConfig {
    const fn new() -> Self {
        Self {
            bytes: [0; NORMALIZED_CAPACITY],
            len: 0,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    fn byte(&mut self, byte: u8) -> Result<(), ConfigError> {
        if self.len == self.bytes.len() {
            return Err(ConfigError::ValueTooLong);
        }
        self.bytes[self.len] = byte;
        self.len += 1;
        Ok(())
    }

    fn raw(&mut self, bytes: &[u8]) -> Result<(), ConfigError> {
        for byte in bytes {
            self.byte(*byte)?;
        }
        Ok(())
    }

    fn number(&mut self, mut value: u64) -> Result<(), ConfigError> {
        let mut digits = [0u8; 20];
        let mut offset = digits.len();
        if value == 0 {
            return self.byte(b'0');
        }
        while value != 0 {
            offset -= 1;
            digits[offset] = b'0' + (value % 10) as u8;
            value /= 10;
        }
        self.raw(&digits[offset..])
    }

    fn quoted(&mut self, bytes: &[u8]) -> Result<(), ConfigError> {
        self.byte(b'"')?;
        for byte in bytes {
            match byte {
                b'"' | b'\\' => {
                    self.byte(b'\\')?;
                    self.byte(*byte)?;
                }
                b'\n' => self.raw(b"\\n")?,
                b'\t' => self.raw(b"\\t")?,
                0..=31 | 127 => return Err(ConfigError::InvalidValue),
                _ => self.byte(*byte)?,
            }
        }
        self.byte(b'"')
    }

    fn list<const N: usize>(&mut self, values: &[Text<N>]) -> Result<(), ConfigError> {
        self.byte(b'[')?;
        for (index, value) in values.iter().enumerate() {
            if index != 0 {
                self.raw(b", ")?;
            }
            self.quoted(value.as_bytes())?;
        }
        self.byte(b']')
    }

    fn syscall_list<const N: usize>(&mut self, values: &[Text<N>]) -> Result<(), ConfigError> {
        self.byte(b'[')?;
        for (index, value) in values.iter().enumerate() {
            if index != 0 {
                self.raw(b", ")?;
            }
            let bytes = value.as_bytes();
            let name = bytes
                .strip_prefix(b"syscall.")
                .ok_or(ConfigError::InvalidContract)?;
            self.quoted(name)?;
        }
        self.byte(b']')
    }
}

fn parse_section(line: &[u8]) -> Result<Section, ConfigError> {
    if line.len() < 3 || line[line.len() - 1] != b']' || line[1..line.len() - 1].contains(&b'[') {
        return Err(ConfigError::InvalidSyntax);
    }
    match &line[1..line.len() - 1] {
        b"instance" => Ok(Section::Instance),
        b"identity" => Ok(Section::Identity),
        b"resources" => Ok(Section::Resources),
        b"execution" => Ok(Section::Execution),
        b"contract" => Ok(Section::Contract),
        b"contract.syscalls" => Ok(Section::ContractSyscalls),
        b"contract.filesystem" => Ok(Section::ContractFilesystem),
        b"contract.network" => Ok(Section::ContractNetwork),
        b"contract.devices" => Ok(Section::ContractDevices),
        b"contract.privileges" => Ok(Section::ContractPrivileges),
        b"contract.process" => Ok(Section::ContractProcess),
        b"contract.kernel" => Ok(Section::ContractKernel),
        b"storage" => Ok(Section::Storage),
        b"volumes" => Ok(Section::Volumes),
        b"devices" => Ok(Section::Devices),
        b"network" => Ok(Section::Network),
        b"access" => Ok(Section::Access),
        b"hooks" => Ok(Section::Hooks),
        b"lifecycle" => Ok(Section::Lifecycle),
        b"audit" => Ok(Section::Audit),
        b"failure" => Ok(Section::Failure),
        _ => Err(ConfigError::UnknownSection),
    }
}

fn parse_field(
    builder: &mut ConfigBuilder,
    section: Section,
    key: &[u8],
    value: &[u8],
) -> Result<(), ConfigError> {
    match (section, key) {
        (Section::Root, b"schema_version") => {
            builder.mark(1 << 0)?;
            builder.schema_version = parse_u64(value)?;
        }
        (Section::Instance, b"id") => {
            builder.mark(1 << 1)?;
            builder.id = parse_string(value)?;
        }
        (Section::Instance, b"root") => {
            builder.mark(1 << 2)?;
            builder.root = parse_string(value)?;
        }
        (Section::Instance, b"init") => {
            builder.mark(1 << 3)?;
            builder.init = parse_string(value)?;
        }
        (Section::Instance, b"args") => {
            builder.mark(1 << 4)?;
            builder.args = parse_list(value)?;
        }
        (Section::Instance, b"environment") => {
            builder.mark(1 << 5)?;
            builder.environment = parse_list(value)?;
        }
        (Section::Instance, b"workdir") => {
            builder.mark(1 << 16)?;
            builder.workdir = parse_string(value)?;
        }
        (Section::Identity, b"hostname") => {
            builder.mark(1 << 17)?;
            builder.hostname = parse_string(value)?;
        }
        (Section::Resources, b"vcpus") => {
            builder.mark(1 << 6)?;
            builder.vcpus = parse_u64(value)?;
        }
        (Section::Resources, b"memory_mib") => {
            builder.mark(1 << 7)?;
            builder.memory_mib = parse_u64(value)?;
        }
        (Section::Execution, b"profile") => {
            builder.mark(1 << 8)?;
            builder.execution_profile = match parse_string::<32>(value)?.as_bytes() {
                b"security" => ExecutionProfile::Security,
                b"balanced" => ExecutionProfile::Balanced,
                b"performance" => ExecutionProfile::Performance,
                _ => return Err(ConfigError::InvalidValue),
            };
        }
        (Section::Execution, b"backend") => {
            builder.mark(1 << 9)?;
            builder.execution_backend = match parse_string::<32>(value)?.as_bytes() {
                b"interpreter" => ExecutionBackend::Interpreter,
                b"ptrace-sysemu" => ExecutionBackend::PtraceSysemu,
                b"ptrace-native" => ExecutionBackend::PtraceNative,
                _ => return Err(ConfigError::InvalidValue),
            };
        }
        (Section::Contract, b"version") => {
            builder.mark(1 << 10)?;
            builder.mark(1u128 << 36)?;
            builder.contract_recipe_version = parse_u64(value)?;
        }
        (Section::Contract, b"catalog") => {
            builder.mark(1u128 << 37)?;
            builder.contract_catalog_version = parse_u64(value)?;
        }
        (Section::Contract, b"default") => {
            builder.mark(1u128 << 38)?;
            builder.contract_default = parse_contract_default(value)?;
        }
        (Section::Contract, b"allow") => {
            builder.mark(1 << 19)?;
            builder.contract_allow = parse_list(value)?;
        }
        (Section::Contract, b"deny") => {
            builder.mark(1 << 20)?;
            builder.contract_deny = parse_list(value)?;
        }
        (Section::Contract, b"digest") => {
            builder.mark(1 << 21)?;
            builder.contract_digest = parse_string(value)?;
        }
        (Section::ContractSyscalls, b"control") => {
            builder.mark(1u128 << 52)?;
            builder.dispatch_mode = match parse_string::<32>(value)?.as_bytes() {
                b"paranoid" => DispatchMode::Paranoid,
                b"selective" => DispatchMode::Selective,
                _ => return Err(ConfigError::InvalidValue),
            };
        }
        (Section::ContractSyscalls, b"default") => {
            builder.mark(1u128 << 53)?;
            parse_contract_default(value)?;
        }
        (Section::ContractSyscalls, b"allow") => {
            builder.mark(1u128 << 47)?;
            builder.contract_exact_allow = parse_syscall_list(value)?;
        }
        (Section::ContractSyscalls, b"deny") => {
            builder.mark(1u128 << 48)?;
            builder.contract_exact_deny = parse_syscall_list(value)?;
        }
        (Section::ContractFilesystem, b"default") => {
            builder.mark(1u128 << 54)?;
            parse_contract_default(value)?;
        }
        (Section::ContractFilesystem, b"read") => {
            builder.mark(1u128 << 55)?;
            builder.filesystem_read = parse_list(value)?;
        }
        (Section::ContractFilesystem, b"write") => {
            builder.mark(1u128 << 56)?;
            builder.filesystem_write = parse_list(value)?;
        }
        (Section::ContractFilesystem, b"create") => {
            builder.mark(1u128 << 57)?;
            builder.filesystem_create = parse_list(value)?;
        }
        (Section::ContractFilesystem, b"deny") => {
            builder.mark(1u128 << 49)?;
            builder.filesystem_write_deny = parse_list(value)?;
        }
        (Section::ContractNetwork, b"default") => {
            builder.mark(1u128 << 58)?;
            parse_contract_default(value)?;
        }
        (Section::ContractNetwork, b"inbound") => {
            builder.mark(1u128 << 42)?;
            builder.ingress_allow = parse_list(value)?;
        }
        (Section::ContractNetwork, b"outbound") => {
            builder.mark(1u128 << 43)?;
            builder.egress_allow = parse_list(value)?;
        }
        (Section::ContractDevices, b"default") => {
            builder.mark(1u128 << 59)?;
            parse_contract_default(value)?;
        }
        (Section::ContractDevices, b"read") => {
            builder.mark(1u128 << 60)?;
            builder.device_read = parse_list(value)?;
        }
        (Section::ContractDevices, b"write") => {
            builder.mark(1u128 << 61)?;
            builder.device_write = parse_list(value)?;
        }
        (Section::ContractDevices, b"create") => {
            builder.mark(1u128 << 62)?;
            builder.device_create = parse_list(value)?;
            builder.filesystem_device_nodes = !builder.device_create.as_slice().is_empty();
        }
        (Section::ContractPrivileges, b"default") => {
            builder.mark(1u128 << 63)?;
            parse_contract_default(value)?;
        }
        (Section::ContractPrivileges, b"setuid") => {
            builder.mark(1u128 << 64)?;
            builder.privilege_setuid = parse_list(value)?;
        }
        (Section::ContractPrivileges, b"setgid") => {
            builder.mark(1u128 << 65)?;
            builder.privilege_setgid = parse_list(value)?;
        }
        (Section::ContractProcess, b"default") => {
            builder.mark(1u128 << 66)?;
            parse_contract_default(value)?;
        }
        (Section::ContractProcess, b"max_processes") => {
            builder.mark(1u128 << 67)?;
            builder.process_max_processes = parse_u64(value)?;
        }
        (Section::ContractProcess, b"max_threads") => {
            builder.mark(1u128 << 68)?;
            builder.process_max_threads = parse_u64(value)?;
        }
        (Section::ContractProcess, b"signals") => {
            builder.mark(1u128 << 69)?;
            builder.process_signals = parse_list(value)?;
        }
        (Section::ContractProcess, b"ptrace") => {
            builder.mark(1u128 << 70)?;
            builder.process_ptrace = parse_allow_deny(value)?;
        }
        (Section::ContractProcess, b"namespace_create") => {
            builder.mark(1u128 << 71)?;
            builder.process_namespace_create = parse_list(value)?;
        }
        (Section::ContractProcess, b"namespace_join") => {
            builder.mark(1u128 << 72)?;
            builder.process_namespace_join = parse_list(value)?;
        }
        (Section::ContractKernel, b"default") => {
            builder.mark(1u128 << 73)?;
            parse_contract_default(value)?;
        }
        (Section::ContractKernel, b"proc_read") => {
            builder.mark(1u128 << 74)?;
            builder.kernel_proc_read = parse_list(value)?;
        }
        (Section::ContractKernel, b"proc_write") => {
            builder.mark(1u128 << 75)?;
            builder.kernel_proc_write = parse_list(value)?;
        }
        (Section::ContractKernel, b"sys_read") => {
            builder.mark(1u128 << 76)?;
            builder.kernel_sys_read = parse_list(value)?;
        }
        (Section::ContractKernel, b"sys_write") => {
            builder.mark(1u128 << 77)?;
            builder.kernel_sys_write = parse_list(value)?;
        }
        (Section::Storage, b"assignments") => {
            builder.mark(1 << 11)?;
            builder.storage = parse_list(value)?;
        }
        (Section::Volumes, b"assignments") => {
            builder.mark(1 << 22)?;
            builder.volumes = parse_list(value)?;
        }
        (Section::Devices, b"assignments") => {
            builder.mark(1 << 12)?;
            builder.devices = parse_list(value)?;
        }
        (Section::Network, b"backend") => {
            builder.mark(1 << 13)?;
            builder.network_backend = match parse_string::<32>(value)?.as_bytes() {
                b"none" => NetworkBackend::None,
                b"mediated" => NetworkBackend::Mediated,
                b"shared" => NetworkBackend::Shared,
                _ => return Err(ConfigError::InvalidValue),
            };
        }
        (Section::Network, b"interface") => {
            builder.mark(1 << 23)?;
            builder.network_interface = parse_string(value)?;
        }
        (Section::Network, b"source") => {
            builder.mark(1 << 24)?;
            builder.network_source = parse_string(value)?;
        }
        (Section::Network, b"ingress_allow") => {
            builder.mark(1 << 25)?;
            builder.ingress_allow = parse_list(value)?;
        }
        (Section::Network, b"ingress_deny") => {
            builder.mark(1 << 26)?;
            builder.ingress_deny = parse_list(value)?;
        }
        (Section::Network, b"egress_allow") => {
            builder.mark(1 << 27)?;
            builder.egress_allow = parse_list(value)?;
        }
        (Section::Network, b"egress_deny") => {
            builder.mark(1 << 28)?;
            builder.egress_deny = parse_list(value)?;
        }
        (Section::Network, b"port_forwards") => {
            builder.mark(1 << 29)?;
            builder.port_forwards = parse_list(value)?;
        }
        (Section::Access, b"uid") => {
            builder.mark(1 << 30)?;
            builder.access_uid = parse_u64(value)?;
        }
        (Section::Access, b"gid") => {
            builder.mark(1 << 31)?;
            builder.access_gid = parse_u64(value)?;
        }
        (Section::Hooks, b"pre_boot") => {
            builder.mark(1u128 << 32)?;
            builder.pre_boot_hooks = parse_list(value)?;
        }
        (Section::Hooks, b"timeout_seconds") => {
            builder.mark(1u128 << 33)?;
            builder.hook_timeout_seconds = parse_u64(value)?;
        }
        (Section::Hooks, b"failure") => {
            builder.mark(1u128 << 34)?;
            builder.hook_failure_policy = match parse_string::<32>(value)?.as_bytes() {
                b"stop" => HookFailurePolicy::Stop,
                b"continue" => HookFailurePolicy::Continue,
                _ => return Err(ConfigError::InvalidValue),
            };
        }
        (Section::Lifecycle, b"stop_timeout_seconds") => {
            builder.mark(1u128 << 35)?;
            builder.stop_timeout_seconds = parse_u64(value)?;
        }
        (Section::Audit, b"mode") => {
            builder.mark(1 << 14)?;
            builder.audit_mode = match parse_string::<32>(value)?.as_bytes() {
                b"stderr" => AuditMode::Stderr,
                b"disabled" => AuditMode::Disabled,
                _ => return Err(ConfigError::InvalidValue),
            };
        }
        (Section::Failure, b"policy") => {
            builder.mark(1 << 15)?;
            builder.failure_policy = match parse_string::<32>(value)?.as_bytes() {
                b"stop" => FailurePolicy::Stop,
                b"fail" => FailurePolicy::Fail,
                _ => return Err(ConfigError::InvalidValue),
            };
        }
        _ => return Err(ConfigError::UnknownField),
    }
    Ok(())
}

fn parse_u64(value: &[u8]) -> Result<u64, ConfigError> {
    if value.is_empty() || !value.iter().all(u8::is_ascii_digit) {
        return Err(ConfigError::InvalidValue);
    }
    let mut number = 0u64;
    for byte in value {
        number = number
            .checked_mul(10)
            .and_then(|value| value.checked_add((byte - b'0') as u64))
            .ok_or(ConfigError::InvalidValue)?;
    }
    Ok(number)
}

fn parse_contract_default(value: &[u8]) -> Result<ContractDefault, ConfigError> {
    match parse_string::<32>(value)?.as_bytes() {
        b"deny" => Ok(ContractDefault::Deny),
        _ => Err(ConfigError::InvalidValue),
    }
}

fn parse_allow_deny(value: &[u8]) -> Result<bool, ConfigError> {
    match parse_string::<32>(value)?.as_bytes() {
        b"allow" => Ok(true),
        b"deny" => Ok(false),
        _ => Err(ConfigError::InvalidValue),
    }
}

fn parse_string<const N: usize>(value: &[u8]) -> Result<Text<N>, ConfigError> {
    if value.len() < 2 || value[0] != b'"' || value[value.len() - 1] != b'"' {
        return Err(ConfigError::InvalidSyntax);
    }
    let mut output = Text::empty();
    let mut index = 1;
    while index < value.len() - 1 {
        let mut byte = value[index];
        if byte == b'\\' {
            index += 1;
            if index >= value.len() - 1 {
                return Err(ConfigError::InvalidSyntax);
            }
            byte = match value[index] {
                b'"' => b'"',
                b'\\' => b'\\',
                b'n' => b'\n',
                b't' => b'\t',
                _ => return Err(ConfigError::InvalidSyntax),
            };
        } else if byte == b'"' || byte < 32 || byte == 127 {
            return Err(ConfigError::InvalidSyntax);
        }
        output.push(byte)?;
        index += 1;
    }
    Ok(output)
}

fn parse_list<const ITEMS: usize, const N: usize>(
    value: &[u8],
) -> Result<TextList<ITEMS, N>, ConfigError> {
    if value.len() < 2 || value[0] != b'[' || value[value.len() - 1] != b']' {
        return Err(ConfigError::InvalidSyntax);
    }
    let mut output = TextList::empty();
    let mut remaining = trim(&value[1..value.len() - 1]);
    while !remaining.is_empty() {
        if remaining[0] != b'"' {
            return Err(ConfigError::InvalidSyntax);
        }
        let mut index = 1;
        let mut escaped = false;
        while index < remaining.len() {
            let byte = remaining[index];
            if byte == b'"' && !escaped {
                break;
            }
            escaped = byte == b'\\' && !escaped;
            index += 1;
        }
        if index == remaining.len() {
            return Err(ConfigError::InvalidSyntax);
        }
        output.push(parse_string(&remaining[..=index])?)?;
        remaining = trim(&remaining[index + 1..]);
        if remaining.is_empty() {
            break;
        }
        if remaining[0] != b',' {
            return Err(ConfigError::InvalidSyntax);
        }
        remaining = trim(&remaining[1..]);
        if remaining.is_empty() {
            return Err(ConfigError::InvalidSyntax);
        }
    }
    Ok(output)
}

fn parse_syscall_list(
    value: &[u8],
) -> Result<TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES>, ConfigError> {
    let names: TextList<CONTRACT_RULE_ITEMS, CONTRACT_RULE_BYTES> = parse_list(value)?;
    let mut selectors = TextList::empty();
    for name in names.as_slice() {
        let bytes = name.as_bytes();
        if bytes.is_empty()
            || bytes.starts_with(b"syscall.")
            || bytes.iter().any(|byte| {
                !byte.is_ascii_lowercase()
                    && !byte.is_ascii_digit()
                    && !matches!(*byte, b'_' | b'-' | b'*')
            })
        {
            return Err(ConfigError::InvalidContract);
        }
        let mut selector = Text::empty();
        for byte in b"syscall.".iter().chain(bytes) {
            selector.push(*byte)?;
        }
        ginux_contract::validate_selector(selector.as_bytes())
            .map_err(|_| ConfigError::InvalidContract)?;
        selectors.push(selector)?;
    }
    Ok(selectors)
}

fn strip_comment(line: &[u8]) -> Result<&[u8], ConfigError> {
    let mut quoted = false;
    let mut escaped = false;
    for (index, byte) in line.iter().enumerate() {
        if *byte == b'"' && !escaped {
            quoted = !quoted;
        }
        if *byte == b'#' && !quoted {
            return Ok(&line[..index]);
        }
        escaped = *byte == b'\\' && !escaped;
        if *byte != b'\\' {
            escaped = false;
        }
    }
    if quoted {
        return Err(ConfigError::InvalidSyntax);
    }
    Ok(line)
}

fn trim(mut bytes: &[u8]) -> &[u8] {
    while bytes.first().is_some_and(u8::is_ascii_whitespace) {
        bytes = &bytes[1..];
    }
    while bytes.last().is_some_and(u8::is_ascii_whitespace) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

fn is_key(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
}

fn validate_identifier(bytes: &[u8]) -> Result<(), ConfigError> {
    if bytes.is_empty()
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(ConfigError::InvalidValue);
    }
    Ok(())
}

fn validate_reference(bytes: &[u8]) -> Result<(), ConfigError> {
    if bytes.is_empty()
        || bytes[0] == b'/'
        || bytes.windows(2).any(|window| window == b"..")
        || !bytes.iter().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/' | b'@')
        })
    {
        return Err(ConfigError::HostAuthority);
    }
    Ok(())
}

fn validate_instance_path(bytes: &[u8]) -> Result<(), ConfigError> {
    if bytes.len() < 2
        || bytes[0] != b'/'
        || bytes.split(|byte| *byte == b'/').any(|part| part == b"..")
        || bytes.contains(&0)
    {
        return Err(ConfigError::InvalidValue);
    }
    Ok(())
}

fn validate_policy_path(bytes: &[u8]) -> Result<(), ConfigError> {
    if bytes == b"/" {
        return Ok(());
    }
    validate_instance_path(bytes)?;
    if bytes.len() > 1
        && (bytes.ends_with(b"/")
            || bytes.windows(2).any(|window| window == b"//")
            || bytes.split(|byte| *byte == b'/').any(|part| part == b"."))
    {
        return Err(ConfigError::InvalidValue);
    }
    Ok(())
}

fn validate_network_attachment(bytes: &[u8]) -> Result<(), ConfigError> {
    if bytes.is_empty()
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(ConfigError::InvalidValue);
    }
    Ok(())
}

fn validate_port(bytes: &[u8], wildcard: bool) -> Result<(), ConfigError> {
    if wildcard && bytes == b"*" {
        return Ok(());
    }
    let port = parse_u64(bytes)?;
    if port == 0 || port > 65_535 {
        return Err(ConfigError::InvalidValue);
    }
    Ok(())
}

fn parse_port(bytes: &[u8]) -> Option<u16> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut value = 0u32;
    for byte in bytes {
        value = value
            .checked_mul(10)?
            .checked_add(u32::from(*byte - b'0'))?;
    }
    if value == 0 || value > u16::MAX as u32 {
        None
    } else {
        Some(value as u16)
    }
}

fn network_rule_matches(rule: &[u8], protocol: NetworkProtocol, port: u16) -> bool {
    let Some(split) = rule.iter().position(|byte| *byte == b':') else {
        return false;
    };
    let rule_protocol = &rule[..split];
    let rule_port = &rule[split + 1..];
    let protocol_matches = match protocol {
        NetworkProtocol::Tcp => rule_protocol == b"tcp",
        NetworkProtocol::Udp => rule_protocol == b"udp",
    };
    protocol_matches && (rule_port == b"*" || parse_port(rule_port) == Some(port))
}

fn validate_network_rule(bytes: &[u8]) -> Result<(), ConfigError> {
    let mut parts = bytes.split(|byte| *byte == b':');
    let protocol = parts.next().ok_or(ConfigError::InvalidValue)?;
    let port = parts.next().ok_or(ConfigError::InvalidValue)?;
    if parts.next().is_some() || !matches!(protocol, b"tcp" | b"udp") {
        return Err(ConfigError::InvalidValue);
    }
    validate_port(port, true)
}

fn validate_port_forward(bytes: &[u8]) -> Result<(), ConfigError> {
    let mut parts = bytes.split(|byte| *byte == b'/');
    let protocol = parts.next().ok_or(ConfigError::InvalidValue)?;
    let attachment = parts.next().ok_or(ConfigError::InvalidValue)?;
    let external = parts.next().ok_or(ConfigError::InvalidValue)?;
    let internal = parts.next().ok_or(ConfigError::InvalidValue)?;
    if parts.next().is_some() || !matches!(protocol, b"tcp" | b"udp") {
        return Err(ConfigError::InvalidValue);
    }
    validate_network_attachment(attachment)?;
    validate_port(external, false)?;
    validate_port(internal, false)
}

type PortForwardParts<'a> = (&'a [u8], &'a [u8], &'a [u8], &'a [u8]);

fn port_forward_parts(bytes: &[u8]) -> Option<PortForwardParts<'_>> {
    let mut parts = bytes.split(|byte| *byte == b'/');
    let result = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        None
    } else {
        Some(result)
    }
}

fn port_forward_conflicts(left: &[u8], right: &[u8]) -> bool {
    let Some((left_protocol, left_attachment, left_external, left_internal)) =
        port_forward_parts(left)
    else {
        return true;
    };
    let Some((right_protocol, right_attachment, right_external, right_internal)) =
        port_forward_parts(right)
    else {
        return true;
    };
    left_protocol == right_protocol
        && ((left_attachment == right_attachment && left_external == right_external)
            || left_internal == right_internal)
}

fn validate_volume(bytes: &[u8]) -> Result<(), ConfigError> {
    let mut parts = bytes.split(|byte| *byte == b':');
    let source = parts.next().ok_or(ConfigError::InvalidValue)?;
    let target = parts.next().ok_or(ConfigError::InvalidValue)?;
    let mode = parts.next().ok_or(ConfigError::InvalidValue)?;
    if parts.next().is_some() || !matches!(mode, b"ro" | b"rw") {
        return Err(ConfigError::InvalidValue);
    }
    validate_instance_path(source)?;
    validate_instance_path(target)?;
    if source == b"/"
        || target == b"/"
        || protected_virtual_path(source)
        || protected_virtual_path(target)
    {
        return Err(ConfigError::HostAuthority);
    }
    Ok(())
}

fn volume_target(bytes: &[u8]) -> Option<&[u8]> {
    let mut parts = bytes.split(|byte| *byte == b':');
    parts.next()?;
    let target = parts.next()?;
    parts.next()?;
    if parts.next().is_some() {
        None
    } else {
        Some(target)
    }
}

fn protected_virtual_path(path: &[u8]) -> bool {
    [b"/proc".as_slice(), b"/sys", b"/dev", b"/run/ginux"]
        .iter()
        .any(|protected| {
            path == *protected
                || (path.starts_with(protected) && path.get(protected.len()).copied() == Some(b'/'))
        })
}

fn validate_instance_command(bytes: &[u8]) -> Result<(), ConfigError> {
    let executable = bytes
        .split(|byte| byte.is_ascii_whitespace())
        .next()
        .ok_or(ConfigError::InvalidValue)?;
    if executable.is_empty()
        || bytes.contains(&0)
        || bytes.contains(&b'\n')
        || bytes.contains(&b'\r')
    {
        return Err(ConfigError::InvalidValue);
    }
    validate_instance_path(executable)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstanceState {
    Created,
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

impl InstanceState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleError {
    InvalidTransition,
}

pub struct Instance {
    config: InstanceConfig,
    state: InstanceState,
    resources_acquired: bool,
    managed_task: bool,
}

impl Instance {
    pub const fn create(config: InstanceConfig) -> Self {
        Self {
            config,
            state: InstanceState::Created,
            resources_acquired: false,
            managed_task: false,
        }
    }

    pub const fn config(&self) -> &InstanceConfig {
        &self.config
    }

    pub const fn state(&self) -> InstanceState {
        self.state
    }

    pub const fn resources_acquired(&self) -> bool {
        self.resources_acquired
    }

    pub const fn managed_task(&self) -> bool {
        self.managed_task
    }

    pub fn start(&mut self) -> Result<(), LifecycleError> {
        if self.state != InstanceState::Created {
            return Err(LifecycleError::InvalidTransition);
        }
        self.state = InstanceState::Starting;
        self.resources_acquired = true;
        self.managed_task = true;
        self.state = InstanceState::Running;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), LifecycleError> {
        if self.state != InstanceState::Running {
            return Err(LifecycleError::InvalidTransition);
        }
        self.state = InstanceState::Stopping;
        self.release();
        self.state = InstanceState::Stopped;
        Ok(())
    }

    pub fn task_exited(&mut self, success: bool) -> Result<(), LifecycleError> {
        if self.state != InstanceState::Running {
            return Err(LifecycleError::InvalidTransition);
        }
        self.release();
        self.state = if success {
            InstanceState::Stopped
        } else {
            InstanceState::Failed
        };
        Ok(())
    }

    pub fn startup_failed(&mut self) -> Result<(), LifecycleError> {
        if !matches!(self.state, InstanceState::Created | InstanceState::Starting) {
            return Err(LifecycleError::InvalidTransition);
        }
        self.release();
        self.state = InstanceState::Failed;
        Ok(())
    }

    fn release(&mut self) {
        self.managed_task = false;
        self.resources_acquired = false;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    const VALID: &str = r#"
schema_version = 1
[instance]
id = "demo"
root = "rocky-10.2"
init = "/runtime-fixture"
args = ["fixture-arg"]
environment = ["GINUX_GATE=4"]
[resources]
vcpus = 1
memory_mib = 128
[execution]
profile = "security"
backend = "interpreter"
[contract]
version = 1
catalog = 2
default = "deny"
allow = []
deny = []
[storage]
assignments = []
[devices]
assignments = []
[network]
backend = "none"
[audit]
mode = "stderr"
[failure]
policy = "stop"
"#;

    #[test]
    fn parses_and_normalizes_versioned_config() {
        let config = InstanceConfig::parse(VALID.as_bytes()).unwrap();
        assert_eq!(config.schema_version, 1);
        assert_eq!(config.id.as_bytes(), b"demo");
        assert_eq!(config.execution_profile, ExecutionProfile::Security);
        assert_eq!(config.dispatch_mode, DispatchMode::Selective);
        assert_eq!(config.contract_digest.as_bytes().len(), 64);
        let normalized = config.normalized().unwrap();
        let reparsed = InstanceConfig::parse(normalized.as_bytes()).unwrap();
        assert_eq!(reparsed, config);
    }

    #[test]
    fn dispatch_mode_is_strict_and_defaults_to_selective() {
        let defaulted = InstanceConfig::parse(VALID.as_bytes()).unwrap();
        assert_eq!(defaulted.dispatch_mode, DispatchMode::Selective);

        let paranoid = VALID.replace(
            "[contract]",
            "[contract.syscalls]\ncontrol = \"paranoid\"\ndefault = \"deny\"\nallow = []\ndeny = []\n[contract]",
        );
        let paranoid = InstanceConfig::parse(paranoid.as_bytes()).unwrap();
        assert_eq!(paranoid.dispatch_mode, DispatchMode::Paranoid);
        assert_eq!(
            InstanceConfig::parse(paranoid.normalized().unwrap().as_bytes())
                .unwrap()
                .dispatch_mode,
            DispatchMode::Paranoid
        );

        let unknown = VALID.replace(
            "[contract]",
            "[contract.syscalls]\ncontrol = \"automatic\"\n[contract]",
        );
        assert_eq!(
            InstanceConfig::parse(unknown.as_bytes()),
            Err(ConfigError::InvalidValue)
        );
    }

    #[test]
    fn self_contained_contract_recipe_round_trips_and_rejects_mixing() {
        let recipe = VALID.replace(
            "[storage]",
            "[contract.filesystem]\ndefault = \"deny\"\nread = [\"/\"]\nwrite = [\"/\"]\ncreate = [\"/\"]\ndeny = []\n[contract.kernel]\ndefault = \"deny\"\nproc_read = [\"/\"]\nproc_write = []\nsys_read = [\"/\"]\nsys_write = []\n[storage]",
        );
        let config = InstanceConfig::parse(recipe.as_bytes()).unwrap();
        assert_eq!(config.contract_recipe_version, 1);
        assert_eq!(config.contract_catalog_version, 2);
        assert_eq!(
            config.contract_read.as_slice()[0].as_bytes(),
            b"filesystem.**"
        );
        assert_eq!(
            config
                .effective_contract()
                .unwrap()
                .operation_rule("filesystem.path.write")
                .unwrap()
                .action,
            ginux_contract::Action::Mediate
        );
        let normalized = config.normalized().unwrap();
        assert_eq!(
            InstanceConfig::parse(normalized.as_bytes()).unwrap(),
            config
        );

        let mixed = recipe.replace("[storage]", "[userspace.contract]\nversion = 1\n[storage]");
        assert_eq!(
            InstanceConfig::parse(mixed.as_bytes()),
            Err(ConfigError::UnknownSection)
        );
    }

    #[test]
    fn contract_inputs_fail_closed_and_digest_tracks_policy() {
        let base = InstanceConfig::parse(VALID.as_bytes()).unwrap();
        let original = base.contract_digest;

        let mut changed = base;
        changed
            .apply_override(b"contract.allow", b"[\"syscall.clock_*\"]")
            .unwrap();
        assert_ne!(changed.contract_digest, original);
        let before_rejected_override = changed;
        assert_eq!(
            changed.apply_override(b"contract.allow", b"[\"syscall.missing*\"]"),
            Err(ConfigError::InvalidContract)
        );
        assert_eq!(changed, before_rejected_override);
        assert_eq!(
            changed.apply_override(b"contract.digest", b"\"forged\""),
            Err(ConfigError::DerivedField)
        );

        let invalid_profile = VALID.replace("catalog = 2", "catalog = 99");
        assert_eq!(
            InstanceConfig::parse(invalid_profile.as_bytes()),
            Err(ConfigError::InvalidContract)
        );
        let mismatched = VALID.replace(
            "[storage]",
            "digest = \"0000000000000000000000000000000000000000000000000000000000000000\"\n[storage]",
        );
        assert_eq!(
            InstanceConfig::parse(mismatched.as_bytes()),
            Err(ConfigError::ContractDigestMismatch)
        );
    }

    #[test]
    fn syscall_globs_expand_against_the_catalog_and_deny_wins() {
        let source = VALID.replace(
            "[storage]",
            "[contract.syscalls]\ncontrol = \"selective\"\ndefault = \"deny\"\nallow = [\"open*\", \"read\"]\ndeny = [\"openat\"]\n[storage]",
        );
        let config = InstanceConfig::parse(source.as_bytes()).unwrap();
        let effective = config.effective_contract().unwrap();
        assert_eq!(
            effective.operation_rule("syscall.open").unwrap().action,
            ginux_contract::Action::Mediate
        );
        assert_eq!(
            effective.operation_rule("syscall.openat").unwrap().action,
            ginux_contract::Action::Deny(1)
        );
        assert_eq!(
            InstanceConfig::parse(config.normalized().unwrap().as_bytes()).unwrap(),
            config
        );

        let unmatched = source.replace("\"open*\"", "\"not_a_real_syscall*\"");
        assert_eq!(
            InstanceConfig::parse(unmatched.as_bytes()),
            Err(ConfigError::InvalidContract)
        );
    }

    #[test]
    fn largest_accepted_config_normalizes_and_round_trips() {
        use std::format;
        use std::string::String;
        use std::vec::Vec;
        // Every field at its capacity. Fields that accept escaped bytes
        // (paths, args, environment keys and values) are filled with quotes;
        // numbers and enums take their longest accepted form.
        let escaped = |count: usize| "\\\"".repeat(count);
        let quoted_list = |items: Vec<String>| {
            items
                .iter()
                .map(|item| format!("\"{item}\""))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let args = quoted_list((0..ARG_ITEMS).map(|_| escaped(ARG_BYTES)).collect());
        let environment = quoted_list(
            (0..ENVIRONMENT_ITEMS)
                .map(|_| format!("{}={}", escaped(1), escaped(ENVIRONMENT_BYTES - 2)))
                .collect(),
        );
        let assignments = quoted_list(
            (0..ASSIGNMENT_ITEMS)
                .map(|i| format!("{}{i}", "d".repeat(ASSIGNMENT_BYTES - 1)))
                .collect(),
        );
        let text = VALID
            .replace(
                "id = \"demo\"",
                &format!("id = \"{}\"", "i".repeat(IDENTIFIER_BYTES)),
            )
            .replace(
                "root = \"rocky-10.2\"",
                &format!("root = \"{}\"", "r".repeat(REFERENCE_BYTES)),
            )
            .replace(
                "init = \"/runtime-fixture\"",
                &format!(
                    "init = \"/{}\"\nworkdir = \"/{}\"",
                    escaped(PATH_BYTES - 1),
                    escaped(PATH_BYTES - 1)
                ),
            )
            .replace("args = [\"fixture-arg\"]", &format!("args = [{args}]"))
            .replace(
                "environment = [\"GINUX_GATE=4\"]",
                &format!("environment = [{environment}]"),
            )
            .replace(
                "assignments = []",
                &format!("assignments = [{assignments}]"),
            )
            .replace("vcpus = 1", "vcpus = 64")
            .replace("memory_mib = 128", "memory_mib = 1048576")
            .replace("profile = \"security\"", "profile = \"performance\"")
            .replace("backend = \"interpreter\"", "backend = \"ptrace-native\"")
            .replace("backend = \"none\"", "backend = \"mediated\"")
            .replace("mode = \"stderr\"", "mode = \"disabled\"");
        let config = InstanceConfig::parse(text.as_bytes()).unwrap();
        assert_eq!(config.args.as_slice().len(), ARG_ITEMS);
        assert_eq!(config.environment.as_slice().len(), ENVIRONMENT_ITEMS);
        assert_eq!(config.devices.as_slice().len(), ASSIGNMENT_ITEMS);
        assert_eq!(config.vcpus, 64);
        assert_eq!(config.network_backend, NetworkBackend::Mediated);
        assert_eq!(config.audit_mode, AuditMode::Disabled);
        let normalized = config.normalized().unwrap();
        // The former fixed 4096-byte buffer could not hold this.
        assert!(normalized.as_bytes().len() > 4096);
        assert_eq!(
            InstanceConfig::parse(normalized.as_bytes()).unwrap(),
            config
        );
    }

    #[test]
    fn input_order_and_comments_do_not_change_effective_config() {
        let commented = VALID.replace(
            "schema_version = 1\n[instance]",
            "# operator comment\nschema_version = 1\n[instance]",
        );
        let resources = "[resources]\nvcpus = 1\nmemory_mib = 128\n";
        let execution = "[execution]\nprofile = \"security\"\nbackend = \"interpreter\"\n";
        let reordered = commented
            .replace(resources, "GATE4_SECTION_PLACEHOLDER\n")
            .replace(execution, resources)
            .replace("GATE4_SECTION_PLACEHOLDER\n", execution);
        let first = InstanceConfig::parse(VALID.as_bytes()).unwrap();
        let second = InstanceConfig::parse(reordered.as_bytes()).unwrap();
        assert_eq!(
            first.normalized().unwrap().as_bytes(),
            second.normalized().unwrap().as_bytes()
        );
    }

    #[test]
    fn rejects_unknown_duplicate_and_missing_fields() {
        let unknown = VALID.replace("id = \"demo\"", "id = \"demo\"\nhost_path = \"/etc\"");
        assert_eq!(
            InstanceConfig::parse(unknown.as_bytes()),
            Err(ConfigError::UnknownField)
        );
        let duplicate = VALID.replace("id = \"demo\"", "id = \"demo\"\nid = \"again\"");
        assert_eq!(
            InstanceConfig::parse(duplicate.as_bytes()),
            Err(ConfigError::DuplicateField)
        );
        let missing = VALID.replace("memory_mib = 128\n", "");
        assert_eq!(
            InstanceConfig::parse(missing.as_bytes()),
            Err(ConfigError::MissingField)
        );
    }

    #[test]
    fn rejects_unsupported_execution_before_start() {
        let performance = VALID.replace("profile = \"security\"", "profile = \"performance\"");
        assert_eq!(
            InstanceConfig::parse(performance.as_bytes()),
            Err(ConfigError::UnsupportedExecution)
        );
    }

    #[test]
    fn accepts_native_ptrace_execution_pair() {
        let native = VALID
            .replace("profile = \"security\"", "profile = \"balanced\"")
            .replace("backend = \"interpreter\"", "backend = \"ptrace-native\"");
        let config = InstanceConfig::parse(native.as_bytes()).unwrap();
        assert_eq!(config.execution_profile, ExecutionProfile::Balanced);
        assert_eq!(config.execution_backend, ExecutionBackend::PtraceNative);
    }

    #[test]
    fn accepts_mediated_network_without_host_authority() {
        let mediated = VALID
            .replace(
                "assignments = []\n[network]",
                "assignments = [\"net0\"]\n[network]",
            )
            .replace("backend = \"none\"", "backend = \"mediated\"");
        let config = InstanceConfig::parse(mediated.as_bytes()).unwrap();
        assert_eq!(config.network_backend, NetworkBackend::Mediated);
        assert_eq!(config.devices.as_slice()[0].as_bytes(), b"net0");
    }

    #[test]
    fn accepts_host_shared_network_as_an_explicit_backend() {
        let shared = VALID.replace("backend = \"none\"", "backend = \"shared\"");
        let config = InstanceConfig::parse(shared.as_bytes()).unwrap();
        assert_eq!(config.network_backend, NetworkBackend::Shared);
        assert!(!config
            .normalized()
            .unwrap()
            .as_bytes()
            .windows(b"backend = \"shared\"".len())
            .any(|window| window == b"backend = \"shared\""));
    }

    #[test]
    fn defaults_omitted_network_to_host_shared() {
        let without_network = VALID.replace("[network]\nbackend = \"none\"\n", "");
        let config = InstanceConfig::parse(without_network.as_bytes()).unwrap();
        assert_eq!(config.network_backend, NetworkBackend::Shared);
    }

    #[test]
    fn parses_complete_hierarchical_schema_and_round_trips() {
        let complete = VALID
            .replace("[resources]", "[identity]\nhostname = \"demo.example\"\n\n[resources]")
            .replace(
                "[storage]",
                "[contract.network]\ndefault = \"deny\"\ninbound = [\"tcp:80\"]\noutbound = [\"tcp:443\", \"udp:53\"]\n\n[storage]",
            )
            .replace(
                "[devices]",
                "[volumes]\nassignments = [\"/srv/data:/data:ro\"]\n\n[devices]",
            )
            .replace(
                "backend = \"none\"",
                "backend = \"shared\"\ninterface = \"eth0:1\"\nsource = \"192.0.2.10\"\nport_forwards = [\"tcp/eth0:1/8080/80\"]",
            )
            .replace(
                "[audit]",
                "[access]\nuid = 1000\ngid = 1000\n\n[hooks]\npre_boot = [\"/usr/local/sbin/prepare-instance --safe\"]\ntimeout_seconds = 20\nfailure = \"stop\"\n\n[lifecycle]\nstop_timeout_seconds = 45\n\n[audit]",
            );
        let config = InstanceConfig::parse(complete.as_bytes()).unwrap();
        assert_eq!(config.hostname.as_bytes(), b"demo.example");
        assert_eq!(config.network_interface.as_bytes(), b"eth0:1");
        assert_eq!(
            config.port_forwards.as_slice()[0].as_bytes(),
            b"tcp/eth0:1/8080/80"
        );
        assert_eq!(config.pre_boot_hooks.as_slice().len(), 1);
        assert!(config.network_allows(true, NetworkProtocol::Tcp, 80));
        assert!(!config.network_allows(true, NetworkProtocol::Tcp, 81));
        assert!(config.network_allows(false, NetworkProtocol::Udp, 53));
        assert!(!config.network_allows(false, NetworkProtocol::Tcp, 25));
        assert_eq!(config.translated_port(NetworkProtocol::Tcp, 80), Some(8080));
        assert_eq!(config.translated_port(NetworkProtocol::Tcp, 81), None);
        let normalized = config.normalized().unwrap();
        let reparsed = InstanceConfig::parse(normalized.as_bytes()).unwrap();
        assert_eq!(
            reparsed.normalized().unwrap().as_bytes(),
            normalized.as_bytes()
        );
    }

    #[test]
    fn command_line_overrides_use_typed_validation() {
        let mut config = InstanceConfig::parse(VALID.as_bytes()).unwrap();
        config.apply_override(b"resources.vcpus", b"4").unwrap();
        config
            .apply_override(b"identity.hostname", b"\"override.example\"")
            .unwrap();
        config
            .apply_override(b"network.backend", b"\"shared\"")
            .unwrap();
        config
            .apply_override(b"network.port_forwards", b"[\"tcp/127.0.0.1/8080/80\"]")
            .unwrap();
        assert_eq!(config.vcpus, 4);
        assert_eq!(config.hostname.as_bytes(), b"override.example");
        assert_eq!(config.port_forwards.as_slice().len(), 1);
        assert_eq!(
            config.apply_override(b"network.interface", b"\"bad interface\""),
            Err(ConfigError::InvalidValue)
        );
        assert_eq!(
            config.apply_override(b"unknown.field", b"\"value\""),
            Err(ConfigError::UnknownField)
        );
    }

    #[test]
    fn rejects_network_controls_without_shared_backend_and_host_hooks() {
        let invalid_network = VALID.replace(
            "backend = \"none\"",
            "backend = \"none\"\ninterface = \"eth0\"",
        );
        assert_eq!(
            InstanceConfig::parse(invalid_network.as_bytes()),
            Err(ConfigError::InvalidValue)
        );
        let host_hook = VALID.replace(
            "[audit]",
            "[hooks]\npre_boot = [\"host:/usr/local/sbin/setup\"]\ntimeout_seconds = 30\nfailure = \"stop\"\n\n[audit]",
        );
        assert_eq!(
            InstanceConfig::parse(host_hook.as_bytes()),
            Err(ConfigError::InvalidValue)
        );
    }

    #[test]
    fn rejects_conflicting_ports_and_unsafe_or_duplicate_volumes() {
        let duplicate_external = VALID.replace(
            "backend = \"none\"",
            "backend = \"shared\"\nport_forwards = [\"tcp/lo/8080/80\", \"tcp/lo/8080/81\"]",
        );
        assert_eq!(
            InstanceConfig::parse(duplicate_external.as_bytes()),
            Err(ConfigError::InvalidValue)
        );

        let duplicate_internal = VALID.replace(
            "backend = \"none\"",
            "backend = \"shared\"\nport_forwards = [\"tcp/lo/8080/80\", \"tcp/lo/8081/80\"]",
        );
        assert_eq!(
            InstanceConfig::parse(duplicate_internal.as_bytes()),
            Err(ConfigError::InvalidValue)
        );

        let unsafe_volume = VALID.replace(
            "[devices]",
            "[volumes]\nassignments = [\"/proc:/data:ro\"]\n\n[devices]",
        );
        assert_eq!(
            InstanceConfig::parse(unsafe_volume.as_bytes()),
            Err(ConfigError::HostAuthority)
        );

        let duplicate_target = VALID.replace(
            "[devices]",
            "[volumes]\nassignments = [\"/srv/one:/data:ro\", \"/srv/two:/data:rw\"]\n\n[devices]",
        );
        assert_eq!(
            InstanceConfig::parse(duplicate_target.as_bytes()),
            Err(ConfigError::InvalidValue)
        );
    }

    #[test]
    fn rejects_implicit_host_authority() {
        let host_root = VALID.replace("root = \"rocky-10.2\"", "root = \"/host/root\"");
        assert_eq!(
            InstanceConfig::parse(host_root.as_bytes()),
            Err(ConfigError::HostAuthority)
        );
        let traversal = VALID.replace("assignments = []", "assignments = [\"../disk\"]");
        assert_eq!(
            InstanceConfig::parse(traversal.as_bytes()),
            Err(ConfigError::HostAuthority)
        );
    }

    #[test]
    fn accepts_digest_pinned_oci_root_reference() {
        let oci = VALID.replace(
            "root = \"rocky-10.2\"",
            "root = \"docker.io/rockylinux/rockylinux@sha256:e372170ca863\"",
        );
        let config = InstanceConfig::parse(oci.as_bytes()).unwrap();
        assert_eq!(
            config.root.as_bytes(),
            b"docker.io/rockylinux/rockylinux@sha256:e372170ca863"
        );
    }

    #[test]
    fn filesystem_policy_is_recursive_and_boundary_aware() {
        use std::format;
        let source = VALID.replace(
            "[storage]",
            "[contract.filesystem]\ndefault = \"deny\"\nread = [\"/\"]\nwrite = [\"/\"]\ncreate = [\"/\"]\ndeny = [\"/ginux-demo/read-only\"]\n\n[contract.devices]\ndefault = \"deny\"\nread = []\nwrite = []\ncreate = []\n\n[contract.privileges]\ndefault = \"deny\"\nsetuid = []\nsetgid = []\n\n[storage]",
        );
        let config = InstanceConfig::parse(source.as_bytes()).unwrap();
        assert!(config.filesystem_write_denied(b"/ginux-demo/read-only"));
        assert!(config.filesystem_write_denied(b"/ginux-demo/read-only/child/value"));
        assert!(!config.filesystem_write_denied(b"/ginux-demo/read-only-other"));
        assert!(config.privilege_setuid.as_slice().is_empty());
        assert!(config.privilege_setgid.as_slice().is_empty());
        assert!(!config.filesystem_device_nodes);
        assert_eq!(
            InstanceConfig::parse(config.normalized().unwrap().as_bytes()).unwrap(),
            config
        );

        for invalid in ["relative", "/path/../escape", "/path//child", "/path/"] {
            let invalid_source = source.replace(
                "deny = [\"/ginux-demo/read-only\"]",
                &format!("deny = [\"{invalid}\"]"),
            );
            assert_eq!(
                InstanceConfig::parse(invalid_source.as_bytes()),
                Err(ConfigError::InvalidValue)
            );
        }
    }

    #[test]
    fn rocky_recipe_owns_bootstrap_and_lockdown_policy() {
        let config = InstanceConfig::parse(include_bytes!(
            "../../../config/instance-rocky-systemd.toml"
        ))
        .unwrap();
        assert_eq!(config.pre_boot_hooks.as_slice().len(), 10);
        assert!(config.pre_boot_hooks.as_slice()[1]
            .as_bytes()
            .windows(b"$6$ginux$".len())
            .any(|window| window == b"$6$ginux$"));
        assert!(config.filesystem_write_denied(b"/ginux-demo/read-only/child"));
        assert!(config.privilege_setuid.as_slice().is_empty());
        assert!(config.privilege_setgid.as_slice().is_empty());
        assert!(!config.filesystem_device_nodes);
    }

    #[test]
    fn lifecycle_reaches_only_documented_states_and_cleans_up() {
        let config = InstanceConfig::parse(VALID.as_bytes()).unwrap();
        let mut instance = Instance::create(config);
        assert_eq!(instance.state(), InstanceState::Created);
        instance.start().unwrap();
        assert_eq!(instance.state(), InstanceState::Running);
        assert!(instance.resources_acquired());
        assert!(instance.managed_task());
        instance.stop().unwrap();
        assert_eq!(instance.state(), InstanceState::Stopped);
        assert!(!instance.resources_acquired());
        assert!(!instance.managed_task());
        assert_eq!(instance.stop(), Err(LifecycleError::InvalidTransition));
    }

    #[test]
    fn startup_and_task_failure_release_resources() {
        let config = InstanceConfig::parse(VALID.as_bytes()).unwrap();
        let mut startup = Instance::create(config);
        startup.startup_failed().unwrap();
        assert_eq!(startup.state(), InstanceState::Failed);
        assert!(!startup.resources_acquired());
        let mut runtime = Instance::create(config);
        runtime.start().unwrap();
        runtime.task_exited(false).unwrap();
        assert_eq!(runtime.state(), InstanceState::Failed);
        assert!(!runtime.resources_acquired());
        assert!(!runtime.managed_task());
    }
}

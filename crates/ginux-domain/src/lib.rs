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
pub enum NetworkBackend {
    None,
    Mediated,
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
pub struct DomainConfig {
    pub schema_version: u64,
    pub id: Text<64>,
    pub root: Text<128>,
    pub init: Text<128>,
    pub args: TextList<7, 128>,
    pub environment: TextList<8, 256>,
    pub workdir: Text<128>,
    pub vcpus: u64,
    pub memory_mib: u64,
    pub execution_profile: ExecutionProfile,
    pub execution_backend: ExecutionBackend,
    pub contract_profile: Text<64>,
    pub storage: TextList<8, 64>,
    pub devices: TextList<8, 64>,
    pub network_backend: NetworkBackend,
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
    Domain,
    Resources,
    Execution,
    Contract,
    Storage,
    Devices,
    Network,
    Audit,
    Failure,
}

struct ConfigBuilder {
    seen: u32,
    schema_version: u64,
    id: Text<64>,
    root: Text<128>,
    init: Text<128>,
    args: TextList<7, 128>,
    environment: TextList<8, 256>,
    workdir: Text<128>,
    vcpus: u64,
    memory_mib: u64,
    execution_profile: ExecutionProfile,
    execution_backend: ExecutionBackend,
    contract_profile: Text<64>,
    storage: TextList<8, 64>,
    devices: TextList<8, 64>,
    network_backend: NetworkBackend,
    audit_mode: AuditMode,
    failure_policy: FailurePolicy,
}

impl ConfigBuilder {
    const fn new() -> Self {
        Self {
            seen: 0,
            schema_version: 0,
            id: Text::empty(),
            root: Text::empty(),
            init: Text::empty(),
            args: TextList::empty(),
            environment: TextList::empty(),
            workdir: Text::empty(),
            vcpus: 0,
            memory_mib: 0,
            execution_profile: ExecutionProfile::Security,
            execution_backend: ExecutionBackend::Interpreter,
            contract_profile: Text::empty(),
            storage: TextList::empty(),
            devices: TextList::empty(),
            network_backend: NetworkBackend::None,
            audit_mode: AuditMode::Stderr,
            failure_policy: FailurePolicy::Stop,
        }
    }

    fn mark(&mut self, bit: u32) -> Result<(), ConfigError> {
        if self.seen & bit != 0 {
            return Err(ConfigError::DuplicateField);
        }
        self.seen |= bit;
        Ok(())
    }

    fn finish(self) -> Result<DomainConfig, ConfigError> {
        const REQUIRED: u32 = (1 << 16) - 1;
        const WORKDIR_BIT: u32 = 1 << 16;
        if self.seen & REQUIRED != REQUIRED {
            return Err(ConfigError::MissingField);
        }
        let workdir = if self.seen & WORKDIR_BIT != 0 {
            self.workdir
        } else {
            Text::from_bytes(b"/work").map_err(|_| ConfigError::InvalidValue)?
        };
        let config = DomainConfig {
            schema_version: self.schema_version,
            id: self.id,
            root: self.root,
            init: self.init,
            args: self.args,
            environment: self.environment,
            workdir,
            vcpus: self.vcpus,
            memory_mib: self.memory_mib,
            execution_profile: self.execution_profile,
            execution_backend: self.execution_backend,
            contract_profile: self.contract_profile,
            storage: self.storage,
            devices: self.devices,
            network_backend: self.network_backend,
            audit_mode: self.audit_mode,
            failure_policy: self.failure_policy,
        };
        config.validate()?;
        Ok(config)
    }
}

impl DomainConfig {
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

    fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedSchema);
        }
        validate_identifier(self.id.as_bytes())?;
        validate_reference(self.root.as_bytes())?;
        validate_domain_path(self.init.as_bytes())?;
        if self.workdir.as_bytes() != b"/" {
            validate_domain_path(self.workdir.as_bytes())?;
        }
        validate_identifier(self.contract_profile.as_bytes())?;
        if !(1..=64).contains(&self.vcpus) || !(16..=1_048_576).contains(&self.memory_mib) {
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
        {
            validate_reference(value.as_bytes())?;
        }
        for value in self.environment.as_slice() {
            let bytes = value.as_bytes();
            let equals = bytes.iter().position(|byte| *byte == b'=');
            if equals == Some(0) || equals.is_none() {
                return Err(ConfigError::InvalidValue);
            }
        }
        Ok(())
    }

    pub fn normalized(&self) -> Result<NormalizedConfig, ConfigError> {
        let mut output = NormalizedConfig::new();
        output.raw(b"schema_version = ")?;
        output.number(self.schema_version)?;
        output.raw(b"\n\n[domain]\nid = ")?;
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
        output.raw(b"\n\n[contract]\nprofile = ")?;
        output.quoted(self.contract_profile.as_bytes())?;
        output.raw(b"\n\n[storage]\nassignments = ")?;
        output.list(self.storage.as_slice())?;
        output.raw(b"\n\n[devices]\nassignments = ")?;
        output.list(self.devices.as_slice())?;
        output.raw(b"\n\n[network]\nbackend = ")?;
        output.quoted(match self.network_backend {
            NetworkBackend::None => b"none",
            NetworkBackend::Mediated => b"mediated",
        })?;
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
}

pub struct NormalizedConfig {
    bytes: [u8; 4096],
    len: usize,
}

impl NormalizedConfig {
    const fn new() -> Self {
        Self {
            bytes: [0; 4096],
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
}

fn parse_section(line: &[u8]) -> Result<Section, ConfigError> {
    if line.len() < 3 || line[line.len() - 1] != b']' || line[1..line.len() - 1].contains(&b'[') {
        return Err(ConfigError::InvalidSyntax);
    }
    match &line[1..line.len() - 1] {
        b"domain" => Ok(Section::Domain),
        b"resources" => Ok(Section::Resources),
        b"execution" => Ok(Section::Execution),
        b"contract" => Ok(Section::Contract),
        b"storage" => Ok(Section::Storage),
        b"devices" => Ok(Section::Devices),
        b"network" => Ok(Section::Network),
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
        (Section::Domain, b"id") => {
            builder.mark(1 << 1)?;
            builder.id = parse_string(value)?;
        }
        (Section::Domain, b"root") => {
            builder.mark(1 << 2)?;
            builder.root = parse_string(value)?;
        }
        (Section::Domain, b"init") => {
            builder.mark(1 << 3)?;
            builder.init = parse_string(value)?;
        }
        (Section::Domain, b"args") => {
            builder.mark(1 << 4)?;
            builder.args = parse_list(value)?;
        }
        (Section::Domain, b"environment") => {
            builder.mark(1 << 5)?;
            builder.environment = parse_list(value)?;
        }
        (Section::Domain, b"workdir") => {
            builder.mark(1 << 16)?;
            builder.workdir = parse_string(value)?;
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
        (Section::Contract, b"profile") => {
            builder.mark(1 << 10)?;
            builder.contract_profile = parse_string(value)?;
        }
        (Section::Storage, b"assignments") => {
            builder.mark(1 << 11)?;
            builder.storage = parse_list(value)?;
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
                _ => return Err(ConfigError::InvalidValue),
            };
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

fn validate_domain_path(bytes: &[u8]) -> Result<(), ConfigError> {
    if bytes.len() < 2
        || bytes[0] != b'/'
        || bytes.split(|byte| *byte == b'/').any(|part| part == b"..")
        || bytes.contains(&0)
    {
        return Err(ConfigError::InvalidValue);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainState {
    Created,
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

impl DomainState {
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

pub struct Domain {
    config: DomainConfig,
    state: DomainState,
    resources_acquired: bool,
    managed_task: bool,
}

impl Domain {
    pub const fn create(config: DomainConfig) -> Self {
        Self {
            config,
            state: DomainState::Created,
            resources_acquired: false,
            managed_task: false,
        }
    }

    pub const fn config(&self) -> &DomainConfig {
        &self.config
    }

    pub const fn state(&self) -> DomainState {
        self.state
    }

    pub const fn resources_acquired(&self) -> bool {
        self.resources_acquired
    }

    pub const fn managed_task(&self) -> bool {
        self.managed_task
    }

    pub fn start(&mut self) -> Result<(), LifecycleError> {
        if self.state != DomainState::Created {
            return Err(LifecycleError::InvalidTransition);
        }
        self.state = DomainState::Starting;
        self.resources_acquired = true;
        self.managed_task = true;
        self.state = DomainState::Running;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), LifecycleError> {
        if self.state != DomainState::Running {
            return Err(LifecycleError::InvalidTransition);
        }
        self.state = DomainState::Stopping;
        self.release();
        self.state = DomainState::Stopped;
        Ok(())
    }

    pub fn task_exited(&mut self, success: bool) -> Result<(), LifecycleError> {
        if self.state != DomainState::Running {
            return Err(LifecycleError::InvalidTransition);
        }
        self.release();
        self.state = if success {
            DomainState::Stopped
        } else {
            DomainState::Failed
        };
        Ok(())
    }

    pub fn startup_failed(&mut self) -> Result<(), LifecycleError> {
        if !matches!(self.state, DomainState::Created | DomainState::Starting) {
            return Err(LifecycleError::InvalidTransition);
        }
        self.release();
        self.state = DomainState::Failed;
        Ok(())
    }

    fn release(&mut self) {
        self.managed_task = false;
        self.resources_acquired = false;
    }
}

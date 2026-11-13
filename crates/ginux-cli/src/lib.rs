use clap::{ArgAction, ArgGroup, Args, Parser, Subcommand};
use ginux_contract::Action;
use ginux_instance::InstanceConfig;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_HELPER: &str = "/usr/local/libexec/ginux-instance-helper";
const HELPER_PROTOCOL: &str = "1";

#[derive(Debug, Parser)]
#[command(
    name = "ginux",
    bin_name = "ginux",
    version,
    about = "Manage Ginux userspace-kernel instances",
    subcommand_required = true,
    arg_required_else_help = true,
    disable_help_subcommand = false
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: PublicCommand,
}

#[derive(Debug, Subcommand)]
pub enum PublicCommand {
    #[command(flatten, next_help_heading = "Configure instances")]
    Configure(ConfigureCommand),
    #[command(flatten, next_help_heading = "Inspect instances")]
    InspectInstance(InspectInstanceCommand),
    #[command(flatten, next_help_heading = "Run instances")]
    Run(RunCommand),
    #[command(flatten, next_help_heading = "Maintain instances")]
    Maintain(MaintainCommand),
    #[command(flatten, next_help_heading = "Inspect recipes")]
    Recipe(RecipeCommand),
}

#[derive(Debug, Subcommand)]
pub enum ConfigureCommand {
    /// Create an instance from the installed default recipe.
    Create(CreateArgs),
    /// Create or replace an instance from a recipe.
    Import(ImportArgs),
    /// Atomically change recipe overrides or host deployment settings.
    Set(SetArgs),
    /// Edit, validate, and atomically apply an instance recipe.
    Edit(NameArgs),
    /// Write an instance recipe to standard output.
    Export(NameArgs),
}

#[derive(Debug, Subcommand)]
pub enum InspectInstanceCommand {
    /// List instances.
    List,
    /// Show instance state and paths.
    Status(NameArgs),
}

#[derive(Debug, Subcommand)]
pub enum RunCommand {
    /// Start an instance.
    Start(StartArgs),
    /// Attach to a running instance console.
    Attach(NameArgs),
    /// Stop an instance.
    Stop(NameArgs),
}

#[derive(Debug, Subcommand)]
pub enum MaintainCommand {
    /// Enforce the configured contract for a stopped instance.
    Lock(NameArgs),
    /// Enable bounded maintenance access for a stopped instance.
    Unlock(NameArgs),
}

#[derive(Debug, Subcommand)]
pub enum RecipeCommand {
    /// Validate and compile a recipe without changing state.
    Validate(FileArgs),
    /// Show a recipe's normalized effective contract.
    Inspect(FileArgs),
    /// Explain one operation's effective contract decision.
    Explain(ExplainArgs),
}

#[derive(Debug, Args)]
pub struct NameArgs {
    #[arg(value_parser = parse_name)]
    pub name: String,
}

#[derive(Debug, Args)]
pub struct FileArgs {
    pub file: PathBuf,
}

#[derive(Debug, Args)]
pub struct ExplainArgs {
    pub file: PathBuf,
    pub operation: String,
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(value_parser = parse_name)]
    pub name: String,
    #[command(flatten)]
    pub deployment: DeploymentArgs,
}

#[derive(Debug, Args)]
pub struct ImportArgs {
    #[arg(value_parser = parse_name)]
    pub name: String,
    pub file: PathBuf,
    #[command(flatten)]
    pub deployment: DeploymentArgs,
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("change")
        .required(true)
        .multiple(true)
        .args(["overrides", "network", "forwards"])
))]
pub struct SetArgs {
    #[arg(value_parser = parse_name)]
    pub name: String,
    #[command(flatten)]
    pub deployment: DeploymentArgs,
}

#[derive(Debug, Args, Default)]
pub struct DeploymentArgs {
    /// Override one recipe key with a TOML value.
    #[arg(long = "set", value_name = "KEY=TOML_VALUE", value_parser = parse_override, action = ArgAction::Append)]
    pub overrides: Vec<String>,
    /// Bind the instance to a host attachment, or use "default".
    #[arg(long, value_name = "ATTACHMENT", value_parser = parse_network)]
    pub network: Option<String>,
    /// Replace port forwarding; use "none" by itself to clear all forwards.
    #[arg(long = "forward", value_name = "SPEC", value_parser = parse_forward, action = ArgAction::Append)]
    pub forwards: Vec<String>,
}

#[derive(Debug, Args)]
pub struct StartArgs {
    #[arg(value_parser = parse_name)]
    pub name: String,
    /// Attach to the instance console after starting it.
    #[arg(long)]
    pub attach: bool,
    /// Enable detailed runtime audit output for this start.
    #[arg(long)]
    pub audit: bool,
}

fn parse_name(value: &str) -> Result<String, String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
    {
        return Err("instance names may contain only letters, digits, '_', '.', and '-'".into());
    }
    Ok(value.into())
}

fn parse_override(value: &str) -> Result<String, String> {
    let Some((key, toml)) = value.split_once('=') else {
        return Err("expected KEY=TOML_VALUE".into());
    };
    if key.is_empty() || toml.is_empty() {
        return Err("expected non-empty KEY=TOML_VALUE".into());
    }
    Ok(value.into())
}

fn parse_network(value: &str) -> Result<String, String> {
    if value == "default"
        || (!value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte)))
    {
        Ok(value.into())
    } else {
        Err("invalid network attachment".into())
    }
}

fn parse_forward(value: &str) -> Result<String, String> {
    if value == "none" {
        return Ok(value.into());
    }
    let mut protocol = None;
    let mut external = None;
    let mut instance = None;
    for field in value.split(',') {
        let Some((key, field_value)) = field.split_once('=') else {
            return Err("expected protocol=...,external_port=...,instance_port=...".into());
        };
        match key {
            "protocol" if protocol.is_none() && matches!(field_value, "tcp" | "udp") => {
                protocol = Some(field_value)
            }
            "external_port" if external.is_none() => external = Some(parse_port(field_value)?),
            "instance_port" if instance.is_none() => instance = Some(parse_port(field_value)?),
            _ => return Err("invalid or duplicate forwarding field".into()),
        }
    }
    if protocol.is_none() || external.is_none() || instance.is_none() {
        return Err("forwarding requires protocol, external_port, and instance_port".into());
    }
    Ok(value.into())
}

fn parse_port(value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| "ports must be integers from 1 through 65535".into())
}

fn validate_deployment(args: &DeploymentArgs) -> Result<(), String> {
    if args.forwards.len() > 1 && args.forwards.iter().any(|value| value == "none") {
        return Err("--forward none conflicts with every other --forward value".into());
    }
    Ok(())
}

pub fn validate_cli(cli: &Cli) -> Result<(), String> {
    match &cli.command {
        PublicCommand::Configure(ConfigureCommand::Create(args)) => {
            validate_deployment(&args.deployment)
        }
        PublicCommand::Configure(ConfigureCommand::Import(args)) => {
            validate_deployment(&args.deployment)
        }
        PublicCommand::Configure(ConfigureCommand::Set(args)) => {
            validate_deployment(&args.deployment)
        }
        _ => Ok(()),
    }
}

pub fn run(cli: Cli) -> Result<i32, String> {
    match cli.command {
        PublicCommand::Configure(ConfigureCommand::Create(args)) => {
            validate_deployment(&args.deployment)?;
            helper_status("create", [OsString::from(args.name)], &args.deployment)
        }
        PublicCommand::Configure(ConfigureCommand::Import(args)) => {
            validate_deployment(&args.deployment)?;
            helper_status(
                "import",
                [OsString::from(args.name), args.file.into_os_string()],
                &args.deployment,
            )
        }
        PublicCommand::Configure(ConfigureCommand::Set(args)) => {
            validate_deployment(&args.deployment)?;
            helper_status("set", [OsString::from(args.name)], &args.deployment)
        }
        PublicCommand::Configure(ConfigureCommand::Edit(args)) => edit_instance(&args.name),
        PublicCommand::Configure(ConfigureCommand::Export(args)) => {
            privileged_helper_status("export", [args.name])
        }
        PublicCommand::InspectInstance(InspectInstanceCommand::List) => {
            privileged_helper_status("list", std::iter::empty::<String>())
        }
        PublicCommand::InspectInstance(InspectInstanceCommand::Status(args)) => {
            privileged_helper_status("status", [args.name])
        }
        PublicCommand::Run(RunCommand::Start(args)) => start_instance(args),
        PublicCommand::Run(RunCommand::Attach(args)) => attach_instance(&args.name),
        PublicCommand::Run(RunCommand::Stop(args)) => privileged_helper_status("stop", [args.name]),
        PublicCommand::Maintain(MaintainCommand::Lock(args)) => {
            privileged_helper_status("lock", [args.name])
        }
        PublicCommand::Maintain(MaintainCommand::Unlock(args)) => {
            privileged_helper_status("unlock", [args.name])
        }
        PublicCommand::Recipe(RecipeCommand::Validate(args)) => validate_recipe(&args.file),
        PublicCommand::Recipe(RecipeCommand::Inspect(args)) => inspect_recipe(&args.file),
        PublicCommand::Recipe(RecipeCommand::Explain(args)) => {
            explain_recipe(&args.file, &args.operation)
        }
    }
}

fn helper_path() -> OsString {
    env::var_os("GINUX_INSTANCE_HELPER").unwrap_or_else(|| DEFAULT_HELPER.into())
}

fn deployment_arguments(args: &DeploymentArgs) -> Vec<OsString> {
    let mut output = Vec::new();
    for value in &args.overrides {
        output.push("--set".into());
        output.push(value.into());
    }
    if let Some(network) = &args.network {
        output.push("--network".into());
        output.push(network.into());
    }
    for value in &args.forwards {
        output.push("--forward".into());
        output.push(value.into());
    }
    output
}

fn helper_status<I, S>(
    command: &str,
    leading: I,
    deployment: &DeploymentArgs,
) -> Result<i32, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut arguments: Vec<OsString> = leading
        .into_iter()
        .map(|value| value.as_ref().to_os_string())
        .collect();
    arguments.extend(deployment_arguments(deployment));
    privileged_helper_status(command, arguments)
}

fn privileged_helper_status<I, S>(command: &str, arguments: I) -> Result<i32, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    status_code(
        Command::new("sudo")
            .arg(helper_path())
            .args(["--protocol", HELPER_PROTOCOL, command])
            .args(arguments),
    )
}

fn status_code(command: &mut Command) -> Result<i32, String> {
    command
        .status()
        .map(|status| status.code().unwrap_or(1))
        .map_err(|error| format!("unable to run command: {error}"))
}

fn start_instance(args: StartArgs) -> Result<i32, String> {
    let mut options = Vec::new();
    if args.attach {
        options.push("--attach");
    }
    if args.audit {
        options.push("--audit");
    }
    let status =
        privileged_helper_status("start", std::iter::once(args.name.as_str()).chain(options))?;
    if status == 0 && args.attach {
        attach_instance(&args.name)
    } else {
        Ok(status)
    }
}

fn current_identity(flag: &str) -> Result<String, String> {
    let output = Command::new("id")
        .arg(flag)
        .output()
        .map_err(|error| format!("unable to determine caller identity: {error}"))?;
    if !output.status.success() {
        return Err("unable to determine caller identity".into());
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|_| "caller identity is not valid UTF-8".into())
}

fn attach_instance(name: &str) -> Result<i32, String> {
    let uid = current_identity("-u")?;
    let gid = current_identity("-g")?;
    let status = Command::new("sudo")
        .arg(helper_path())
        .args([
            "--protocol",
            HELPER_PROTOCOL,
            "authorize-attach",
            name,
            &uid,
            &gid,
        ])
        .stdout(Stdio::null())
        .status()
        .map_err(|error| format!("unable to authorize console attachment: {error}"))?;
    if !status.success() {
        return Ok(status.code().unwrap_or(1));
    }
    let error = Command::new(helper_path())
        .args(["--protocol", HELPER_PROTOCOL, "attach", name])
        .exec();
    Err(format!("unable to attach console: {error}"))
}

fn edit_instance(name: &str) -> Result<i32, String> {
    let mut path = env::temp_dir();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before the Unix epoch".to_owned())?
        .as_nanos();
    path.push(format!("ginux-edit-{}-{nonce}.toml", std::process::id()));
    let result = edit_instance_at(name, &path);
    let _ = fs::remove_file(&path);
    result
}

fn edit_instance_at(name: &str, path: &Path) -> Result<i32, String> {
    let file = fs::File::create_new(path)
        .map_err(|error| format!("unable to create temporary recipe: {error}"))?;
    let export = Command::new("sudo")
        .arg(helper_path())
        .args(["--protocol", HELPER_PROTOCOL, "export", name])
        .stdout(Stdio::from(file))
        .status()
        .map_err(|error| format!("unable to export instance recipe: {error}"))?;
    if !export.success() {
        return Ok(export.code().unwrap_or(1));
    }
    let editor = env::var_os("VISUAL")
        .or_else(|| env::var_os("EDITOR"))
        .unwrap_or_else(|| "vi".into());
    let edited = Command::new(editor)
        .arg(path)
        .status()
        .map_err(|error| format!("unable to start editor: {error}"))?;
    if !edited.success() {
        return Ok(edited.code().unwrap_or(1));
    }
    privileged_helper_status("import", [OsStr::new(name), path.as_os_str()])
}

fn load_recipe(path: &Path) -> Result<(InstanceConfig, ginux_contract::EffectiveContract), String> {
    let source =
        fs::read(path).map_err(|error| format!("unable to read {}: {error}", path.display()))?;
    let config = InstanceConfig::parse(&source)
        .map_err(|error| format!("configuration rejected: {error}"))?;
    let contract = config
        .effective_contract()
        .map_err(|error| format!("configuration rejected: {error}"))?;
    Ok((config, contract))
}

fn validate_recipe(path: &Path) -> Result<i32, String> {
    let (config, contract) = load_recipe(path)?;
    println!(
        "Contract valid: instance={} catalog={} digest={} operations={}",
        text(config.id.as_bytes()),
        ginux_contract::CATALOG_VERSION,
        text(contract.digest()),
        contract.rules().len()
    );
    Ok(0)
}

fn inspect_recipe(path: &Path) -> Result<i32, String> {
    let (config, contract) = load_recipe(path)?;
    let mut emulate = 0;
    let mut mediate = 0;
    let mut deny = 0;
    for rule in contract.rules() {
        match rule.action {
            Action::Emulate => emulate += 1,
            Action::Mediate => mediate += 1,
            Action::Deny(_) => deny += 1,
        }
    }
    println!(
        "Contract: instance={} catalog={} digest={}",
        text(config.id.as_bytes()),
        ginux_contract::CATALOG_VERSION,
        text(contract.digest())
    );
    println!(
        "Decisions: emulate={emulate} mediate={mediate} deny={deny} total={}",
        contract.rules().len()
    );
    Ok(0)
}

fn explain_recipe(path: &Path, operation: &str) -> Result<i32, String> {
    let (_, contract) = load_recipe(path)?;
    let Some(rule) = contract.operation_rule(operation) else {
        eprintln!(
            "Contract operation is not present in catalog version {}.",
            ginux_contract::CATALOG_VERSION
        );
        return Ok(2);
    };
    println!("Operation: {}", rule.operation.id);
    println!("Decision: {}", rule.action.name());
    println!(
        "Rule: {} source={} index={}",
        rule.provenance.name(),
        rule.source,
        rule.source_rule
    );
    println!("Status: {}", rule.operation.status.name());
    println!("Constraint: {}", rule.operation.constraint);
    println!("Limitation: {}", rule.operation.limitation);
    Ok(0)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub fn print_error(error: &str) {
    let _ = writeln!(io::stderr(), "ginux: {error}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    fn parse(arguments: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(arguments)
    }

    #[test]
    fn command_model_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn public_commands_are_exactly_the_approved_set() {
        let names: Vec<_> = Cli::command()
            .get_subcommands()
            .map(|command| command.get_name().to_owned())
            .collect();
        assert_eq!(
            names,
            [
                "create", "import", "set", "edit", "export", "list", "status", "start", "attach",
                "stop", "lock", "unlock", "validate", "inspect", "explain"
            ]
        );
        assert!(!names.iter().any(|name| name == "diff" || name == "native"));
    }

    #[test]
    fn deployment_arguments_preserve_order() {
        let cli = parse(&[
            "ginux",
            "create",
            "rocky",
            "--set",
            "resources.vcpus=2",
            "--set",
            "resources.memory_mib=1024",
            "--forward",
            "protocol=tcp,external_port=8080,instance_port=80",
            "--forward",
            "protocol=udp,external_port=5353,instance_port=53",
        ])
        .unwrap();
        let PublicCommand::Configure(ConfigureCommand::Create(args)) = cli.command else {
            panic!("unexpected command")
        };
        assert_eq!(
            args.deployment.overrides,
            ["resources.vcpus=2", "resources.memory_mib=1024"]
        );
        assert_eq!(args.deployment.forwards.len(), 2);
        validate_deployment(&args.deployment).unwrap();
    }

    #[test]
    fn set_requires_at_least_one_change() {
        let error = parse(&["ginux", "set", "rocky"]).unwrap_err();
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn forwarding_none_conflicts_with_other_values() {
        let cli = parse(&[
            "ginux",
            "set",
            "rocky",
            "--forward",
            "none",
            "--forward",
            "protocol=tcp,external_port=8080,instance_port=80",
        ])
        .unwrap();
        assert!(validate_cli(&cli).is_err());
    }

    #[test]
    fn missing_extra_and_unknown_arguments_are_parse_errors() {
        for arguments in [
            &["ginux", "status"] as &[&str],
            &["ginux", "list", "extra"],
            &["ginux", "start", "rocky", "--unknown"],
            &["ginux", "diff", "left", "right"],
            &["ginux", "native", "config", "root"],
        ] {
            assert_eq!(parse(arguments).unwrap_err().exit_code(), 2);
        }
    }
}

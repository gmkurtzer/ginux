#![no_std]

#[cfg(test)]
extern crate std;

pub const MAX_ARGUMENTS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arity {
    minimum: u8,
    maximum: u8,
}

impl Arity {
    pub const fn exact(count: u8) -> Self {
        assert!(count as usize <= MAX_ARGUMENTS);
        Self {
            minimum: count,
            maximum: count,
        }
    }

    pub const fn range(minimum: u8, maximum: u8) -> Self {
        assert!(minimum <= maximum);
        assert!(maximum as usize <= MAX_ARGUMENTS);
        Self { minimum, maximum }
    }

    const fn accepts(self, count: usize) -> bool {
        count >= self.minimum as usize && count <= self.maximum as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Command<'schema> {
    pub name: &'schema [u8],
    pub usage: &'schema [u8],
    pub about: &'schema [u8],
    pub id: Option<u16>,
    pub arguments: Arity,
    pub subcommands: &'schema [Command<'schema>],
}

impl<'schema> Command<'schema> {
    pub const fn leaf(
        name: &'schema [u8],
        usage: &'schema [u8],
        about: &'schema [u8],
        id: u16,
        arguments: Arity,
    ) -> Self {
        Self {
            name,
            usage,
            about,
            id: Some(id),
            arguments,
            subcommands: &[],
        }
    }

    pub const fn group(
        name: &'schema [u8],
        usage: &'schema [u8],
        about: &'schema [u8],
        subcommands: &'schema [Command<'schema>],
    ) -> Self {
        Self {
            name,
            usage,
            about,
            id: None,
            arguments: Arity::exact(0),
            subcommands,
        }
    }

    fn child(&self, name: &[u8]) -> Option<&Command<'schema>> {
        self.subcommands.iter().find(|child| child.name == name)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Invocation<'input> {
    id: u16,
    arguments: [&'input [u8]; MAX_ARGUMENTS],
    argument_count: usize,
}

impl<'input> Invocation<'input> {
    pub const fn id(self) -> u16 {
        self.id
    }

    pub fn arguments(&self) -> &[&'input [u8]] {
        &self.arguments[..self.argument_count]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseOutcome<'schema, 'input> {
    Empty,
    Help(&'schema Command<'schema>),
    Invoke(Invocation<'input>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseErrorKind {
    UnknownCommand,
    MissingArguments,
    TooManyArguments,
    TooManyTokens,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseError<'schema> {
    kind: ParseErrorKind,
    command: &'schema Command<'schema>,
}

impl<'schema> ParseError<'schema> {
    pub const fn kind(self) -> ParseErrorKind {
        self.kind
    }

    pub const fn command(self) -> &'schema Command<'schema> {
        self.command
    }
}

pub fn parse<'schema, 'input>(
    root: &'schema Command<'schema>,
    input: &'input [u8],
) -> Result<ParseOutcome<'schema, 'input>, ParseError<'schema>> {
    let mut words = input
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|word| !word.is_empty());
    let Some(first) = words.next() else {
        return Ok(ParseOutcome::Empty);
    };
    if first == b"help" {
        return resolve_help(root, words);
    }

    let Some(mut command) = root.child(first) else {
        return Err(ParseError {
            kind: ParseErrorKind::UnknownCommand,
            command: root,
        });
    };
    let mut arguments = [b"" as &[u8]; MAX_ARGUMENTS];
    let mut argument_count = 0;
    for word in words {
        if word == b"--help" || (word == b"help" && command.id.is_none()) {
            return Ok(ParseOutcome::Help(command));
        }
        if argument_count == 0 {
            if let Some(child) = command.child(word) {
                command = child;
                continue;
            }
        }
        if command.id.is_none() {
            return Err(ParseError {
                kind: ParseErrorKind::UnknownCommand,
                command,
            });
        }
        if argument_count == MAX_ARGUMENTS {
            return Err(ParseError {
                kind: ParseErrorKind::TooManyTokens,
                command,
            });
        }
        arguments[argument_count] = word;
        argument_count += 1;
    }
    let Some(id) = command.id else {
        return Ok(ParseOutcome::Help(command));
    };
    if !command.arguments.accepts(argument_count) {
        return Err(ParseError {
            kind: if argument_count < command.arguments.minimum as usize {
                ParseErrorKind::MissingArguments
            } else {
                ParseErrorKind::TooManyArguments
            },
            command,
        });
    }
    Ok(ParseOutcome::Invoke(Invocation {
        id,
        arguments,
        argument_count,
    }))
}

fn resolve_help<'schema, 'input>(
    root: &'schema Command<'schema>,
    words: impl Iterator<Item = &'input [u8]>,
) -> Result<ParseOutcome<'schema, 'input>, ParseError<'schema>> {
    let mut command = root;
    for word in words {
        let Some(child) = command.child(word) else {
            return Err(ParseError {
                kind: ParseErrorKind::UnknownCommand,
                command,
            });
        };
        command = child;
    }
    Ok(ParseOutcome::Help(command))
}

pub trait HelpWriter {
    fn write(&mut self, bytes: &[u8]);
}

pub fn write_help(command: &Command<'_>, output: &mut impl HelpWriter) {
    output.write(command.about);
    output.write(b"\r\n\r\nUsage:\r\n  ");
    output.write(command.usage);
    output.write(b"\r\n");
    if !command.subcommands.is_empty() {
        output.write(b"\r\nAvailable Commands:\r\n");
        for child in command.subcommands {
            output.write(b"  ");
            output.write(child.usage);
            output.write(b"\r\n      ");
            output.write(child.about);
            output.write(b"\r\n");
        }
    }
}

pub fn write_error(error: ParseError<'_>, output: &mut impl HelpWriter) {
    output.write(match error.kind {
        ParseErrorKind::UnknownCommand => b"Error: unknown command",
        ParseErrorKind::MissingArguments => b"Error: missing arguments",
        ParseErrorKind::TooManyArguments | ParseErrorKind::TooManyTokens => {
            b"Error: too many arguments"
        }
    });
    output.write(b"\r\nUsage:\r\n  ");
    output.write(error.command.usage);
    output.write(b"\r\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{string::String, vec::Vec};

    const START: Command<'static> = Command::leaf(
        b"start",
        b"container start NAME",
        b"Start one container",
        3,
        Arity::exact(1),
    );
    const SET: Command<'static> = Command::leaf(
        b"set",
        b"container set NAME FIELD=VALUE...",
        b"Set container fields",
        4,
        Arity::range(2, 5),
    );
    const CONTAINER_CHILDREN: &[Command<'static>] = &[START, SET];
    const CONTAINER: Command<'static> = Command::group(
        b"container",
        b"container COMMAND",
        b"Manage containers",
        CONTAINER_CHILDREN,
    );
    const STATUS: Command<'static> =
        Command::leaf(b"status", b"status", b"Show status", 2, Arity::exact(0));
    const ROOT_CHILDREN: &[Command<'static>] = &[STATUS, CONTAINER];
    const ROOT: Command<'static> =
        Command::group(b"ginux", b"ginux COMMAND", b"Ginux shell", ROOT_CHILDREN);

    #[test]
    fn parses_nested_commands_and_normalizes_whitespace() {
        let ParseOutcome::Invoke(invocation) =
            parse(&ROOT, b"  container\tset  demo cpu  1 ").unwrap()
        else {
            panic!("expected invocation");
        };
        assert_eq!(invocation.id(), 4);
        let expected: &[&[u8]] = &[b"demo", b"cpu", b"1"];
        assert_eq!(invocation.arguments(), expected);
    }

    #[test]
    fn parses_multiple_container_settings() {
        let ParseOutcome::Invoke(invocation) = parse(
            &ROOT,
            b"container set demo init=/sbin/init cpu=1 memory-mib=128 autostart=true",
        )
        .unwrap() else {
            panic!("expected invocation");
        };
        let expected: &[&[u8]] = &[
            b"demo",
            b"init=/sbin/init",
            b"cpu=1",
            b"memory-mib=128",
            b"autostart=true",
        ];
        assert_eq!(invocation.arguments(), expected);
    }

    #[test]
    fn resolves_root_and_nested_help() {
        assert!(matches!(
            parse(&ROOT, b"help"),
            Ok(ParseOutcome::Help(&ROOT))
        ));
        assert!(matches!(
            parse(&ROOT, b"container help"),
            Ok(ParseOutcome::Help(&CONTAINER))
        ));
        assert!(matches!(
            parse(&ROOT, b"help container start"),
            Ok(ParseOutcome::Help(&START))
        ));
        let ParseOutcome::Invoke(invocation) = parse(&ROOT, b"container start help").unwrap()
        else {
            panic!("expected invocation");
        };
        let expected: &[&[u8]] = &[b"help"];
        assert_eq!(invocation.arguments(), expected);
    }

    #[test]
    fn reports_scope_and_arity_errors() {
        let unknown = parse(&ROOT, b"container launch demo").unwrap_err();
        assert_eq!(unknown.kind(), ParseErrorKind::UnknownCommand);
        assert_eq!(unknown.command(), &CONTAINER);
        let missing = parse(&ROOT, b"container start").unwrap_err();
        assert_eq!(missing.kind(), ParseErrorKind::MissingArguments);
        assert_eq!(missing.command(), &START);
        let extra = parse(&ROOT, b"status now").unwrap_err();
        assert_eq!(extra.kind(), ParseErrorKind::TooManyArguments);
        assert_eq!(extra.command(), &STATUS);
    }

    struct Buffer(Vec<u8>);

    impl HelpWriter for Buffer {
        fn write(&mut self, bytes: &[u8]) {
            self.0.extend_from_slice(bytes);
        }
    }

    #[test]
    fn generates_help_from_the_command_tree() {
        let mut output = Buffer(Vec::new());
        write_help(&CONTAINER, &mut output);
        let output = String::from_utf8(output.0).unwrap();
        assert!(output.contains("Usage:\r\n  container COMMAND"));
        assert!(output.contains("container start NAME"));
        assert!(output.contains("Start one container"));
    }
}

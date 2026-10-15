#![no_std]

#[cfg(test)]
extern crate std;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arity {
    minimum: u8,
    maximum: Option<u8>,
}

impl Arity {
    pub const fn exact(count: u8) -> Self {
        Self {
            minimum: count,
            maximum: Some(count),
        }
    }

    pub const fn range(minimum: u8, maximum: u8) -> Self {
        assert!(minimum <= maximum);
        Self {
            minimum,
            maximum: Some(maximum),
        }
    }

    pub const fn at_least(minimum: u8) -> Self {
        Self {
            minimum,
            maximum: None,
        }
    }

    const fn accepts(self, count: usize) -> bool {
        count >= self.minimum as usize
            && match self.maximum {
                Some(maximum) => count <= maximum as usize,
                None => true,
            }
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
    arguments: &'input [u8],
    argument_count: usize,
}

impl<'input> Invocation<'input> {
    pub const fn id(self) -> u16 {
        self.id
    }

    pub const fn arguments(&self) -> Arguments<'input> {
        Arguments {
            input: self.arguments,
            remaining: self.argument_count,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arguments<'input> {
    input: &'input [u8],
    remaining: usize,
}

impl<'input> Arguments<'input> {
    pub const fn len(self) -> usize {
        self.remaining
    }

    pub const fn is_empty(self) -> bool {
        self.remaining == 0
    }

    pub fn get(mut self, index: usize) -> Option<&'input [u8]> {
        self.nth(index)
    }

    pub fn first(self) -> Option<&'input [u8]> {
        self.get(0)
    }
}

impl<'input> Iterator for Arguments<'input> {
    type Item = &'input [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let start = self
            .input
            .iter()
            .position(|byte| !byte.is_ascii_whitespace())?;
        self.input = &self.input[start..];
        let end = self
            .input
            .iter()
            .position(|byte| byte.is_ascii_whitespace())
            .unwrap_or(self.input.len());
        let word = &self.input[..end];
        self.input = &self.input[end..];
        self.remaining -= 1;
        Some(word)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for Arguments<'_> {}

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
    let mut argument_start = input.len();
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
        if argument_count == 0 {
            argument_start = word.as_ptr() as usize - input.as_ptr() as usize;
        }
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
        arguments: &input[argument_start..],
        argument_count,
    }))
}

pub fn complete(root: &Command<'_>, input: &[u8], output: &mut [u8]) -> usize {
    let ends_with_space = input.last().is_some_and(u8::is_ascii_whitespace);
    let mut words = input
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|word| !word.is_empty())
        .peekable();
    let mut command = root;
    let mut partial = b"" as &[u8];
    while let Some(word) = words.next() {
        if words.peek().is_none() && !ends_with_space {
            partial = word;
            break;
        }
        let Some(child) = command.child(word) else {
            return 0;
        };
        command = child;
    }
    if command.id.is_some() {
        return 0;
    }
    let mut match_count = 0;
    let mut common = b"" as &[u8];
    for child in command
        .subcommands
        .iter()
        .filter(|child| child.name.starts_with(partial))
    {
        if match_count == 0 {
            common = child.name;
        } else {
            let length = common
                .iter()
                .zip(child.name)
                .position(|(left, right)| left != right)
                .unwrap_or(common.len().min(child.name.len()));
            common = &common[..length];
        }
        match_count += 1;
    }
    if match_count == 0 || common.len() <= partial.len() {
        return 0;
    }
    let suffix = &common[partial.len()..];
    let add_space = match_count == 1;
    let length = suffix.len() + usize::from(add_space);
    if length > output.len() {
        return 0;
    }
    output[..suffix.len()].copy_from_slice(suffix);
    if add_space {
        output[suffix.len()] = b' ';
    }
    length
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
        ParseErrorKind::TooManyArguments => b"Error: too many arguments",
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
        Arity::at_least(2),
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
        assert!(invocation.arguments().eq(expected.iter().copied()));
    }

    #[test]
    fn parses_multiple_container_settings() {
        let ParseOutcome::Invoke(invocation) = parse(
            &ROOT,
            b"container set demo image=registry.example/alpine@sha256:abc init=/bin/sh cpu=1 memory-mib=128 autostart=true",
        )
        .unwrap() else {
            panic!("expected invocation");
        };
        let expected: &[&[u8]] = &[
            b"demo",
            b"image=registry.example/alpine@sha256:abc",
            b"init=/bin/sh",
            b"cpu=1",
            b"memory-mib=128",
            b"autostart=true",
        ];
        assert!(invocation.arguments().eq(expected.iter().copied()));

        let ParseOutcome::Invoke(invocation) =
            parse(&ROOT, b"container set demo a=1 b=2 c=3 d=4 e=5 f=6 g=7 h=8").unwrap()
        else {
            panic!("expected variadic invocation");
        };
        assert_eq!(invocation.arguments().len(), 9);
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
        assert!(invocation.arguments().eq(expected.iter().copied()));
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

    #[test]
    fn completes_root_and_nested_command_names() {
        let mut output = [0_u8; 32];
        let length = complete(&ROOT, b"cont", &mut output);
        assert_eq!(&output[..length], b"ainer ");
        let length = complete(&ROOT, b"container sta", &mut output);
        assert_eq!(&output[..length], b"rt ");
        assert_eq!(complete(&ROOT, b"container start ", &mut output), 0);
        assert_eq!(complete(&ROOT, b"missing", &mut output), 0);
    }
}

# Gate 4 domain configuration and lifecycle

Gate 4 uses a strict, versioned TOML configuration. Every approved field is
required so the effective domain authority is visible. Unknown sections,
unknown or duplicate fields, unsupported schema versions, host paths, and
unsupported execution or network backends fail before managed code starts.

The initial supported execution choice is:

```toml
[execution]
profile = "security"
backend = "interpreter"
```

`performance` and `ptrace-sysemu` are recognized future choices but are
rejected in Gate 4 because no approved performance backend exists yet. Storage
and device assignments are logical identifiers, never host paths or
descriptors. Their operational backends arrive in Gate 6.

The parser intentionally accepts the documented TOML subset: basic quoted
strings, decimal integers, arrays of basic strings, comments, and the named
tables in the schema. TOML features outside that subset fail closed. Ginux
prints one canonical effective configuration regardless of harmless input
ordering, whitespace, or comments.

The lifecycle is:

```text
created -> starting -> running -> stopping -> stopped
                         |
                         `-> failed
```

Startup and runtime failures release all acquired resources. Repeated stop or
invalid transitions are rejected. One Ginux host process owns one domain in
the initial milestone; Ginux never becomes host PID 1.

Run the human checkpoint with:

```sh
make demo-gate-4
```

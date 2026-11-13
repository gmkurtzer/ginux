# Concurrent instances and isolation

Ginux can run independently named Rocky Linux systemd instances concurrently.
Each instance has its own portable recipe, deployment state, runtime contract,
root and writable state, supervisor, console, audit log, virtual PID space,
synthetic kernel views, and lifecycle. A lifecycle or contract change addressed
to one instance does not apply to another.

The deterministic Gate 17 workflow boots two instances from the same immutable
OCI image and verifies:

- distinct supervisor and runtime identities, hostnames, boot IDs, virtual
  process views, contract material, consoles, and audit identity;
- private root writes and empty backing directories for the on-demand `/proc`
  and `/sys` models;
- rejection of peer host PIDs through an instance's virtual signal interface;
- duplicate-start rejection, contract lock isolation, supervisor-loss cleanup,
  restart, stale-record recovery, and peer survival; and
- shared-host network behavior, including one winner for an ordinary port
  conflict and simultaneous internal binds translated to different external
  ports.

Run the automated workflow or its operator-readable wrapper with the pinned
host and OCI images already cached:

```sh
make test-multi-instance
make demo-gate-17
```

## Network boundary

The current backend is intentionally host-shared. Instances do not receive
Linux network namespaces, private interfaces, or private address spaces.
Without a translated port, two instances binding the same host address and port
have ordinary first-successful-bind semantics. `--forward` gives an instance a
separate external port while preserving its configured internal port, and its
deployment state remains owned by that instance.

This is a tested two-instance baseline, not a density, orchestration, or
network-namespace claim. The host kernel, Ginux executable and configuration,
and operator remain trusted. Isolation claims apply to supported Ginux
interfaces and the checked-in x86-64 Rocky configuration; they do not imply
protection from a compromised host kernel or administrator.

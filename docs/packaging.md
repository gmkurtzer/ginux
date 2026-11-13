# Rocky Linux RPM packaging

Ginux ships as one x86-64 RPM for Rocky Linux 10. The package contains the
public CLI, the private runtime, image materializer and privileged helper, and
complete example recipes. It does not pull an image, create an instance, start
a service, add sudo policy, or change SELinux or firewall policy at install
time.

## Build

From a clean committed checkout on macOS or Rocky Linux:

```sh
make rpm-source
make rpm
make test-rpm-metadata
```

`make rpm-source` creates a versioned source archive and a separate vendored
Rust dependency archive under `build/rpm/SOURCES/`. Network access is allowed
only for this explicit preparation step. `make rpm` performs `rpmbuild -ba`
without network access. On macOS it uses the repository's Rocky Linux 10 QEMU
builder and copies the resulting artifacts back to:

```text
build/rpm/RPMS/x86_64/
build/rpm/SRPMS/
```

Inspect the result with:

```sh
rpm -qip build/rpm/RPMS/x86_64/ginux-*.rpm
rpm -qpl build/rpm/RPMS/x86_64/ginux-*.rpm
```

## Install and verify

```sh
sudo dnf install build/rpm/RPMS/x86_64/ginux-*.rpm
sudo rpm -V ginux
ginux --version
ginux validate /etc/ginux/examples/rocky10-systemd.toml
```

The public command is `/usr/bin/ginux`. Private implementation programs are
under `/usr/libexec/ginux/` and are not public CLI aliases. Example recipes are
installed as `%config(noreplace)` under `/etc/ginux/examples/`.

## State and lifecycle

Ginux stores verified OCI manifests, configuration objects and compressed
layers by digest under `/var/lib/ginux/cache/oci/blobs/sha256/`. It expands the
layers directly into `/var/lib/ginux/instances/NAME/root/`; it does not retain
a redundant `rootfs.tar`. Instance configuration, deployment state, logs and
expanded roots live with the named instance. PIDs, FIFOs and control endpoints
are recreated under `/run/ginux/instances/NAME/`.

Reinstallation and upgrade preserve recipes changed by an operator, cached OCI
blobs and all instance state. Package removal deletes package-owned programs
and unchanged examples but never recursively deletes `/var/lib/ginux`:

```sh
sudo dnf remove ginux
sudo dnf install build/rpm/RPMS/x86_64/ginux-*.rpm
```

To deliberately purge retained state after removing Ginux, first verify that
no Ginux processes remain, then remove `/var/lib/ginux` explicitly. This is an
operator action and is never performed by the RPM.

Run the complete clean Rocky package lifecycle test with `make test-rpm`.

## Initial limitations

The first package supports Rocky Linux 10 on x86-64 only. It is not signed and
is not published through a Ginux repository. Repository publication, package
signing, other RPM-family distributions and additional architectures are
outside this packaging gate.

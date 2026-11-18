use ginux_fs_policy::{Capability, CapabilityKind, ObjectId, Rights};
use std::fs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Barrier};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    base: PathBuf,
    instance: PathBuf,
    peer: PathBuf,
    host: PathBuf,
}

#[derive(Debug, Eq, PartialEq)]
struct ProtectedObject {
    bytes: Vec<u8>,
    device: u64,
    inode: u64,
    mode: u32,
}

impl ProtectedObject {
    fn capture(path: &Path) -> Self {
        let metadata = fs::metadata(path).unwrap();
        Self {
            bytes: fs::read(path).unwrap(),
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
        }
    }
}

struct HostEffectOracle {
    peer_marker: PathBuf,
    host_marker: PathBuf,
    peer_before: ProtectedObject,
    host_before: ProtectedObject,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct Effects {
    protected_read: bool,
    write: bool,
    metadata: bool,
    handle: bool,
    transient: bool,
}

impl HostEffectOracle {
    fn new(fixture: &Fixture) -> Self {
        let peer_marker = fixture.peer.join("marker");
        let host_marker = fixture.host.join("marker");
        Self {
            peer_before: ProtectedObject::capture(&peer_marker),
            host_before: ProtectedObject::capture(&host_marker),
            peer_marker,
            host_marker,
        }
    }

    fn protected_read(&self, observed: &[u8]) -> bool {
        observed == self.peer_before.bytes || observed == self.host_before.bytes
    }

    fn effects(&self, observed_read: Option<&[u8]>, transient: bool) -> Effects {
        let peer_after = ProtectedObject::capture(&self.peer_marker);
        let host_after = ProtectedObject::capture(&self.host_marker);
        Effects {
            protected_read: observed_read.is_some_and(|bytes| self.protected_read(bytes)),
            write: peer_after.bytes != self.peer_before.bytes
                || host_after.bytes != self.host_before.bytes,
            metadata: peer_after.mode != self.peer_before.mode
                || host_after.mode != self.host_before.mode,
            handle: (peer_after.device, peer_after.inode)
                != (self.peer_before.device, self.peer_before.inode)
                || (host_after.device, host_after.inode)
                    != (self.host_before.device, self.host_before.inode),
            transient,
        }
    }

    fn assert_clean(&self) {
        assert_eq!(self.effects(None, false), Effects::default());
    }
}

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "ginux-fs-oracle-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        let instance = base.join("instance");
        let peer = base.join("peer");
        let host = base.join("host-only");
        fs::create_dir_all(&instance).unwrap();
        fs::create_dir_all(&peer).unwrap();
        fs::create_dir_all(&host).unwrap();
        fs::write(instance.join("marker"), b"instance-marker").unwrap();
        fs::write(peer.join("marker"), b"peer-marker").unwrap();
        fs::write(host.join("marker"), b"host-marker").unwrap();
        Self {
            base,
            instance,
            peer,
            host,
        }
    }

    fn object(path: &Path) -> ObjectId {
        let metadata = fs::metadata(path).unwrap();
        ObjectId {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }

    fn marker(&self, path: &Path) -> Vec<u8> {
        fs::read(path.join("marker")).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

#[test]
fn root_capability_does_not_follow_replaced_provisioning_name() {
    let fixture = Fixture::new();
    let oracle = HostEffectOracle::new(&fixture);
    let acquired = Fixture::object(&fixture.instance);
    let moved = fixture.base.join("instance-acquired");
    fs::rename(&fixture.instance, &moved).unwrap();
    fs::create_dir(&fixture.instance).unwrap();
    fs::write(fixture.instance.join("marker"), b"decoy-marker").unwrap();

    assert_eq!(Fixture::object(&moved), acquired);
    assert_ne!(Fixture::object(&fixture.instance), acquired);
    assert_eq!(fixture.marker(&moved), b"instance-marker");
    assert_eq!(fixture.marker(&fixture.instance), b"decoy-marker");
    assert_eq!(fixture.marker(&fixture.peer), b"peer-marker");
    assert_eq!(fixture.marker(&fixture.host), b"host-marker");
    oracle.assert_clean();
}

#[test]
fn peer_host_and_stale_generations_never_gain_authority() {
    let fixture = Fixture::new();
    let oracle = HostEffectOracle::new(&fixture);
    let instance_object = Fixture::object(&fixture.instance);
    let peer_object = Fixture::object(&fixture.peer);
    let host_object = Fixture::object(&fixture.host);
    let capability = Capability::new(
        CapabilityKind::Root,
        17,
        19,
        23,
        instance_object,
        Rights::LOOKUP.union(Rights::READ),
    );

    assert!(capability.authorizes(17, 19, 23, Rights::READ));
    assert!(!capability.authorizes(17, 19, 24, Rights::READ));
    assert!(!capability.authorizes(17, 20, 23, Rights::READ));
    assert_ne!(capability.object, peer_object);
    assert_ne!(capability.object, host_object);
    assert_eq!(fixture.marker(&fixture.peer), b"peer-marker");
    assert_eq!(fixture.marker(&fixture.host), b"host-marker");
    oracle.assert_clean();
}

#[test]
fn acquired_object_survives_a_deterministic_path_replacement_race() {
    let fixture = Fixture::new();
    let oracle = HostEffectOracle::new(&fixture);
    let original_marker = Fixture::object(&fixture.instance.join("marker"));
    let peer_before = fixture.marker(&fixture.peer);
    let host_before = fixture.marker(&fixture.host);
    let mut acquired = File::open(fixture.instance.join("marker")).unwrap();
    let acquired_identity = ObjectId {
        device: acquired.metadata().unwrap().dev(),
        inode: acquired.metadata().unwrap().ino(),
    };
    let replacement_path = fixture.instance.clone();
    let moved_path = fixture.base.join("instance-acquired-race");
    let ready = Arc::new(Barrier::new(2));
    let replaced = Arc::new(Barrier::new(2));
    let worker_ready = Arc::clone(&ready);
    let worker_replaced = Arc::clone(&replaced);

    let worker = thread::spawn(move || {
        worker_ready.wait();
        fs::rename(&replacement_path, &moved_path).unwrap();
        fs::create_dir(&replacement_path).unwrap();
        fs::write(replacement_path.join("marker"), b"race-decoy").unwrap();
        worker_replaced.wait();
    });

    ready.wait();
    replaced.wait();
    acquired.seek(SeekFrom::Start(0)).unwrap();
    let mut observed = Vec::new();
    acquired.read_to_end(&mut observed).unwrap();
    worker.join().unwrap();

    assert_eq!(acquired_identity, original_marker);
    assert_eq!(observed, b"instance-marker");
    assert_eq!(fixture.marker(&fixture.instance), b"race-decoy");
    assert_eq!(fixture.marker(&fixture.peer), peer_before);
    assert_eq!(fixture.marker(&fixture.host), host_before);
    oracle.assert_clean();
}

#[test]
fn lexical_prefix_collision_never_matches_object_authority() {
    let fixture = Fixture::new();
    let oracle = HostEffectOracle::new(&fixture);
    let collision = fixture.base.join("instance-peer");
    fs::create_dir(&collision).unwrap();
    fs::write(collision.join("marker"), b"collision-marker").unwrap();
    let root = Fixture::object(&fixture.instance);
    let capability = Capability::new(CapabilityKind::Root, 17, 19, 23, root, Rights::LOOKUP);

    let instance_text = fixture.instance.to_string_lossy();
    assert!(collision
        .to_string_lossy()
        .starts_with(instance_text.as_ref()));
    assert!(!collision.starts_with(&fixture.instance));
    assert_ne!(Fixture::object(&collision), capability.object);
    assert_eq!(fixture.marker(&collision), b"collision-marker");
    assert_eq!(fixture.marker(&fixture.peer), b"peer-marker");
    assert_eq!(fixture.marker(&fixture.host), b"host-marker");
    oracle.assert_clean();
}

#[test]
fn oracle_detects_every_protected_effect_class() {
    let fixture = Fixture::new();
    let oracle = HostEffectOracle::new(&fixture);
    oracle.assert_clean();

    let exposed = fs::read(fixture.peer.join("marker")).unwrap();
    fs::write(fixture.peer.join("marker"), b"injected-write").unwrap();
    fs::set_permissions(
        fixture.peer.join("marker"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let replaced = fixture.host.join("replaced-marker");
    fs::rename(fixture.host.join("marker"), &replaced).unwrap();
    fs::write(fixture.host.join("marker"), b"host-marker").unwrap();

    let transient = fixture.peer.join("transient-effect");
    let (inspect_tx, inspect_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let observed_path = transient.clone();
    let observer = thread::spawn(move || {
        inspect_rx.recv().unwrap();
        result_tx.send(observed_path.exists()).unwrap();
    });
    fs::write(&transient, b"temporary").unwrap();
    inspect_tx.send(()).unwrap();
    let transient_observed = result_rx.recv().unwrap();
    fs::remove_file(transient).unwrap();
    observer.join().unwrap();

    assert_eq!(
        oracle.effects(Some(&exposed), transient_observed),
        Effects {
            protected_read: true,
            write: true,
            metadata: true,
            handle: true,
            transient: true,
        }
    );
}

use ginux_fs_policy::{Capability, CapabilityKind, ObjectId, Rights};
use std::fs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture {
    base: PathBuf,
    instance: PathBuf,
    peer: PathBuf,
    host: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base =
            std::env::temp_dir().join(format!("ginux-fs-oracle-{}-{nonce}", std::process::id()));
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
}

#[test]
fn peer_host_and_stale_generations_never_gain_authority() {
    let fixture = Fixture::new();
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
}

#[test]
fn acquired_object_survives_a_deterministic_path_replacement_race() {
    let fixture = Fixture::new();
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
}

#[test]
fn lexical_prefix_collision_never_matches_object_authority() {
    let fixture = Fixture::new();
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
}

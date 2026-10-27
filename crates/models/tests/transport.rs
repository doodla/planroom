//! The live transport is reqwest over rustls with the ring provider (DR-39): neither OpenSSL nor
//! aws-lc enters the tree, and the client is built on ring's provider. Building a client sends
//! nothing.

use std::time::Duration;

use planroom_models::transport::Live;
use rustls::crypto::CryptoProvider;

#[test]
fn the_live_transport_is_rustls_with_ring_and_nothing_else() {
    let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"))
        .expect("Cargo.lock");
    let packages: Vec<&str> =
        lock.lines().filter_map(|line| line.strip_prefix("name = \"")?.strip_suffix('"')).collect();
    for banned in ["openssl", "openssl-sys", "native-tls", "aws-lc-rs", "aws-lc-sys", "aws-lc-fips-sys"] {
        assert!(!packages.contains(&banned), "{banned} is in Cargo.lock; DR-39 names rustls with ring");
    }
    assert!(packages.contains(&"ring"), "ring is missing from Cargo.lock");

    Live::new(Duration::from_secs(1)).expect("the live client builds");
    let provider = CryptoProvider::get_default().expect("the transport installs a crypto provider");
    let ring = rustls::crypto::ring::default_provider();
    let groups = |p: &CryptoProvider| p.kx_groups.iter().map(|g| g.name()).collect::<Vec<_>>();
    let suites = |p: &CryptoProvider| p.cipher_suites.iter().map(|s| s.suite()).collect::<Vec<_>>();
    assert_eq!(groups(provider), groups(&ring));
    assert_eq!(suites(provider), suites(&ring));
}

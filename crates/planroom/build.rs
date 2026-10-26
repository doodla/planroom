//! Records the target triple for `version --json`.

fn main() {
    let target = std::env::var("TARGET").expect("cargo sets TARGET for build scripts");
    println!("cargo:rustc-env=PLANROOM_TARGET={target}");
    println!("cargo:rerun-if-changed=build.rs");
}

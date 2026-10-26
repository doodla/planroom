//! `cargo xtask fixture OUT [--no-packs]`: `examples/build-fixture OUT`, then
//! `OUT/.planroom/config.toml` loading `examples/packs/ruby` (DR-13). The pack loader reads that
//! config once it exists; until then the path is ready and unused.

use std::fs::{self, DirBuilder};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::Path;

use crate::util;

/// Build the fixture into `out`; returns `build-fixture`'s JSON map of repo to branch heads.
pub fn build(root: &Path, out: &Path, packs: bool) -> Result<String, String> {
    let out = std::env::current_dir().map_err(|e| e.to_string())?.join(out);
    let script = root.join("examples/build-fixture");
    let out_str = out.to_str().ok_or("OUT must be a UTF-8 path")?;
    let heads =
        util::capture(root, "python3", &[script.to_str().ok_or("the checkout path isn't UTF-8")?, out_str])?;
    if packs {
        let dir = out.join(".planroom");
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?;
        let pack = root.join("examples/packs/ruby");
        let pack = serde_json::to_string(pack.to_str().ok_or("the checkout path isn't UTF-8")?)
            .map_err(|e| e.to_string())?;
        let config = dir.join("config.toml");
        fs::write(&config, format!("packs = [{pack}]\n"))
            .map_err(|e| format!("{}: {e}", config.display()))?;
        fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }
    Ok(heads)
}

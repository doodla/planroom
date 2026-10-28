//! The engine's own repo-ownership check (DR-78): the effective user ID, read once at start-up and
//! handed to the git adapter, which refuses every git call in a repo another user owns before git
//! runs, whatever git's version or its backports. A scan skips such a repo with `repo_skipped`,
//! reason `other_owner`.

/// Hand the git adapter this process's effective user ID, once, at start-up.
pub fn register() {
    planroom_extract::git::set_euid(euid());
}

/// The effective user ID, or in a debug build the test seam's stand-in.
fn euid() -> u32 {
    #[cfg(debug_assertions)]
    if let Some(uid) = test_euid() {
        return uid;
    }
    // SAFETY: geteuid has no preconditions and always succeeds.
    unsafe { libc::geteuid() }
}

/// The test seam for the ownership check: in a debug build only, `PLANROOM_TEST_EUID` stands in
/// for the effective user ID, so a test can plant a repo another user owns without root. Release
/// builds compile this out, so no setting a person or an agent can reach turns the check off,
/// and the mod sets it empty for every engine run, which reads as unset (DR-92).
#[cfg(debug_assertions)]
fn test_euid() -> Option<u32> {
    std::env::var("PLANROOM_TEST_EUID").ok()?.parse().ok()
}

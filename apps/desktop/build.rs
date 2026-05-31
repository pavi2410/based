//! Build script: bake the short git commit SHA and build timestamp into the binary.
//!
//! `BASED_GIT_SHA` is used by the About window to show `vX.Y.Z · commit abcdefg`.
//! `BASED_BUILD_TIMESTAMP` is the Unix epoch (seconds) at compile time, formatted
//! by the About window into a human-readable date.
//!
//! Released binaries get a real SHA because `actions/checkout@v6` leaves `.git`
//! present at build time; out-of-tree (tarball) builds with no `.git` fall back
//! to `"unknown"`.

use std::process::Command;

fn main() {
    let sha = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());

    let build_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    println!("cargo:rustc-env=BASED_GIT_SHA={sha}");
    println!("cargo:rustc-env=BASED_BUILD_TIMESTAMP={build_ts}");

    // Re-run when HEAD moves or a ref changes (branch checkout, new commit, etc.).
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
}

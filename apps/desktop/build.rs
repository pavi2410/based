//! Build script: bake the short git commit SHA into the binary as `BASED_GIT_SHA`.
//!
//! Used by the About window to show `vX.Y.Z - commit abcdefg`. Released binaries
//! get a real SHA because `actions/checkout@v6` leaves `.git` present at build
//! time; out-of-tree (tarball) builds with no `.git` fall back to `"unknown"`.

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
    println!("cargo:rustc-env=BASED_GIT_SHA={sha}");
    // Re-run when HEAD moves or a ref changes (branch checkout, new commit, etc.).
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
}

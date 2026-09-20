use std::process::Command;

// Resolves the version string baked into the binary as GATHERS_VERSION.
// Priority: GATHERS_VERSION env var (set by CI / Docker builds, which have no
// .git available) > `git describe --tags` > the Cargo package version.
fn main() {
    let version = std::env::var("GATHERS_VERSION")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(git_describe)
        .unwrap_or_else(|| std::env::var("CARGO_PKG_VERSION").unwrap_or_default());

    println!("cargo:rustc-env=GATHERS_VERSION={}", version.trim());
    println!("cargo:rerun-if-env-changed=GATHERS_VERSION");
    // Best effort for local builds: re-run when HEAD moves or a tag is added.
    // A new tag on the same commit isn't caught; `touch server/build.rs` then.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs/tags");
    println!("cargo:rerun-if-changed=../.git/packed-refs");
}

fn git_describe() -> Option<String> {
    let out = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!v.is_empty()).then_some(v)
}

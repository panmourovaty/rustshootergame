use std::process::Command;

fn main() {
    // Re-run only when the commit pointer changes (cheap check).
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");
    // Also re-run when the caller overrides the hash via env (CI / Docker).
    println!("cargo:rerun-if-env-changed=GIT_HASH");

    // Prefer the value injected by CI / Docker build-arg; fall back to `git`;
    // final fallback is "unknown" (e.g. when .git is absent in the container).
    let hash = std::env::var("GIT_HASH")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_HASH={}", hash);
}

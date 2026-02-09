use std::process::Command;

fn main() {
    // Re-run if these env vars change or if git HEAD changes
    println!("cargo:rerun-if-env-changed=APTLY_VERSION");
    println!("cargo:rerun-if-env-changed=APTLY_GIT_SHA");
    println!("cargo:rerun-if-env-changed=APTLY_BUILD_DATE");
    println!("cargo:rerun-if-changed=../../.git/HEAD");

    let version = std::env::var("APTLY_VERSION").unwrap_or_else(|_| {
        git_describe().unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
    });

    let git_sha = std::env::var("APTLY_GIT_SHA")
        .unwrap_or_else(|_| git_short_sha().unwrap_or_else(|| "unknown".to_string()));

    let build_date = std::env::var("APTLY_BUILD_DATE").unwrap_or_else(|_| "unknown".to_string());

    println!("cargo:rustc-env=APTLY_VERSION={version}");
    println!("cargo:rustc-env=APTLY_GIT_SHA={git_sha}");
    println!("cargo:rustc-env=APTLY_BUILD_DATE={build_date}");
}

fn git_describe() -> Option<String> {
    Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

fn git_short_sha() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

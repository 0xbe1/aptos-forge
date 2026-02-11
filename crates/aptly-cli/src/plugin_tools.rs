use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const MOVE_DECOMPILER_BIN: &str = "move-decompiler";
const APTOS_TRACER_BIN: &str = "aptos-tracer";
const APTOS_SCRIPT_COMPOSE_BIN: &str = "aptos-script-compose";

#[derive(Debug, Clone, Serialize)]
pub struct PluginStatus {
    pub name: String,
    pub description: String,
    pub installed: bool,
    pub binary_path: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorCheck {
    pub name: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginDoctorReport {
    pub plugin: PluginStatus,
    pub checks: Vec<DoctorCheck>,
    pub install_hint: Option<String>,
}

impl PluginDoctorReport {
    pub fn all_ok(&self) -> bool {
        self.checks.iter().all(|check| check.ok)
    }
}

#[derive(Debug, Clone)]
struct DiscoveryResult {
    path: Option<PathBuf>,
    source: Option<String>,
}

pub fn discover_move_decompiler(explicit_bin: Option<&str>) -> PluginStatus {
    let result = resolve_move_decompiler(explicit_bin);
    let installed = result
        .path
        .as_ref()
        .map(|path| path.is_file())
        .unwrap_or(false);

    PluginStatus {
        name: "move-decompiler".to_owned(),
        description:
            "Optional Aptos Move decompiler plugin backed by aptos-core third_party toolchain"
                .to_owned(),
        installed,
        binary_path: result.path.map(|path| path.display().to_string()),
        source: result.source,
    }
}

pub fn doctor_move_decompiler(explicit_bin: Option<&str>) -> PluginDoctorReport {
    let plugin = discover_move_decompiler(explicit_bin);
    let mut checks = Vec::new();

    checks.push(DoctorCheck {
        name: "binary_discovered".to_owned(),
        ok: plugin.installed,
        message: if plugin.installed {
            format!(
                "Found move-decompiler at {}",
                plugin.binary_path.as_deref().unwrap_or_default()
            )
        } else {
            "move-decompiler binary not found".to_owned()
        },
    });

    if let Some(path) = plugin.binary_path.as_deref() {
        let executable = is_executable(path);
        checks.push(DoctorCheck {
            name: "binary_executable".to_owned(),
            ok: executable,
            message: if executable {
                "Binary is executable".to_owned()
            } else {
                "Binary exists but is not executable".to_owned()
            },
        });

        let runnable = Command::new(path)
            .arg("--help")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        checks.push(DoctorCheck {
            name: "binary_runnable".to_owned(),
            ok: runnable,
            message: if runnable {
                "move-decompiler responds to --help".to_owned()
            } else {
                "Failed to run move-decompiler --help".to_owned()
            },
        });
    }

    let install_hint = if checks.iter().all(|check| check.ok) {
        None
    } else {
        Some(move_decompiler_install_hint())
    };

    PluginDoctorReport {
        plugin,
        checks,
        install_hint,
    }
}

pub fn run_move_decompiler(explicit_bin: Option<&str>, args: &[String]) -> Result<()> {
    let result = resolve_move_decompiler(explicit_bin);
    let path = result.path.ok_or_else(|| {
        anyhow!(
            "move-decompiler plugin is not installed.\n{}",
            move_decompiler_install_hint()
        )
    })?;

    if args.is_empty() {
        return Err(anyhow!(
            "missing move-decompiler arguments. Pass args after `--`, e.g. `aptly decompile raw -- --help`"
        ));
    }

    let status = Command::new(&path)
        .args(args)
        .status()
        .with_context(|| format!("failed to execute {}", path.display()))?;
    if !status.success() {
        return Err(anyhow!("move-decompiler exited with status {status}"));
    }

    Ok(())
}

pub fn move_decompiler_install_hint() -> String {
    [
        "Install move-decompiler from aptos-core and put it on PATH (or pass --decompiler-bin):",
        "  git clone https://github.com/aptos-labs/aptos-core",
        "  cd aptos-core",
        "  cargo build -p move-decompiler --release",
        "  export PATH=$PWD/target/release:$PATH",
        "or pass --decompiler-bin /path/to/move-decompiler",
    ]
    .join("\n")
}

pub fn discover_aptos_tracer(explicit_bin: Option<&str>) -> PluginStatus {
    let result = resolve_aptos_tracer(explicit_bin);
    let installed = result
        .path
        .as_ref()
        .map(|path| path.is_file())
        .unwrap_or(false);

    PluginStatus {
        name: "aptos-tracer".to_owned(),
        description: "Optional Aptos transaction tracer plugin backed by sentio aptos-core"
            .to_owned(),
        installed,
        binary_path: result.path.map(|path| path.display().to_string()),
        source: result.source,
    }
}

pub fn doctor_aptos_tracer(explicit_bin: Option<&str>) -> PluginDoctorReport {
    let plugin = discover_aptos_tracer(explicit_bin);
    let mut checks = Vec::new();

    checks.push(DoctorCheck {
        name: "binary_discovered".to_owned(),
        ok: plugin.installed,
        message: if plugin.installed {
            format!(
                "Found aptos-tracer at {}",
                plugin.binary_path.as_deref().unwrap_or_default()
            )
        } else {
            "aptos-tracer binary not found".to_owned()
        },
    });

    if let Some(path) = plugin.binary_path.as_deref() {
        let executable = is_executable(path);
        checks.push(DoctorCheck {
            name: "binary_executable".to_owned(),
            ok: executable,
            message: if executable {
                "Binary is executable".to_owned()
            } else {
                "Binary exists but is not executable".to_owned()
            },
        });

        let runnable = Command::new(path)
            .arg("--help")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        checks.push(DoctorCheck {
            name: "binary_runnable".to_owned(),
            ok: runnable,
            message: if runnable {
                "aptos-tracer responds to --help".to_owned()
            } else {
                "Failed to run aptos-tracer --help".to_owned()
            },
        });
    }

    let install_hint = if checks.iter().all(|check| check.ok) {
        None
    } else {
        Some(aptos_tracer_install_hint())
    };

    PluginDoctorReport {
        plugin,
        checks,
        install_hint,
    }
}

pub fn resolve_aptos_tracer_bin(explicit_bin: Option<&str>) -> Result<PathBuf> {
    let result = resolve_aptos_tracer(explicit_bin);
    let path = result.path.ok_or_else(|| {
        anyhow!(
            "aptos-tracer plugin is not installed.\n{}",
            aptos_tracer_install_hint()
        )
    })?;

    if !path.is_file() {
        return Err(anyhow!(
            "aptos-tracer binary was resolved but does not exist: {}",
            path.display()
        ));
    }

    Ok(path)
}

pub fn aptos_tracer_install_hint() -> String {
    [
        "Install aptos-tracer from sentio aptos-core and put it on PATH:",
        "  git clone --branch sentio/dev-2026-0210 https://github.com/sentioxyz/aptos-core.git",
        "  cd aptos-core",
        "  cargo build --locked --profile cli -p aptos-tracer",
        "  export PATH=$PWD/target/cli:$PATH",
        "or run `aptly tx trace <tx_version_or_hash> --local-tracer /path/to/aptos-tracer`",
    ]
    .join("\n")
}

pub fn discover_aptos_script_compose(explicit_bin: Option<&str>) -> PluginStatus {
    let result = resolve_aptos_script_compose(explicit_bin);
    let installed = result
        .path
        .as_ref()
        .map(|path| path.is_file())
        .unwrap_or(false);

    PluginStatus {
        name: "aptos-script-compose".to_owned(),
        description: "Optional Aptos script composer plugin for batched call bytecode generation"
            .to_owned(),
        installed,
        binary_path: result.path.map(|path| path.display().to_string()),
        source: result.source,
    }
}

pub fn doctor_aptos_script_compose(explicit_bin: Option<&str>) -> PluginDoctorReport {
    let plugin = discover_aptos_script_compose(explicit_bin);
    let mut checks = Vec::new();

    checks.push(DoctorCheck {
        name: "binary_discovered".to_owned(),
        ok: plugin.installed,
        message: if plugin.installed {
            format!(
                "Found aptos-script-compose at {}",
                plugin.binary_path.as_deref().unwrap_or_default()
            )
        } else {
            "aptos-script-compose binary not found".to_owned()
        },
    });

    if let Some(path) = plugin.binary_path.as_deref() {
        let executable = is_executable(path);
        checks.push(DoctorCheck {
            name: "binary_executable".to_owned(),
            ok: executable,
            message: if executable {
                "Binary is executable".to_owned()
            } else {
                "Binary exists but is not executable".to_owned()
            },
        });

        let runnable = Command::new(path)
            .arg("--help")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        checks.push(DoctorCheck {
            name: "binary_runnable".to_owned(),
            ok: runnable,
            message: if runnable {
                "aptos-script-compose responds to --help".to_owned()
            } else {
                "Failed to run aptos-script-compose --help".to_owned()
            },
        });
    }

    let install_hint = if checks.iter().all(|check| check.ok) {
        None
    } else {
        Some(aptos_script_compose_install_hint())
    };

    PluginDoctorReport {
        plugin,
        checks,
        install_hint,
    }
}

pub fn aptos_script_compose_install_hint() -> String {
    [
        "Install aptos-script-compose from aptly and put it on PATH (or pass --script-compose-bin):",
        "  cargo build -p aptos-script-compose --release",
        "  export PATH=$PWD/target/release:$PATH",
        "or pass --script-compose-bin /path/to/aptos-script-compose",
    ]
    .join("\n")
}

fn resolve_move_decompiler(explicit_bin: Option<&str>) -> DiscoveryResult {
    if let Some(bin) = explicit_bin {
        if !bin.trim().is_empty() {
            return DiscoveryResult {
                path: Some(PathBuf::from(bin)),
                source: Some("flag:--decompiler-bin".to_owned()),
            };
        }
    }

    if let Some(path) = find_in_path(MOVE_DECOMPILER_BIN) {
        return DiscoveryResult {
            path: Some(path),
            source: Some("PATH".to_owned()),
        };
    }

    DiscoveryResult {
        path: None,
        source: None,
    }
}

fn resolve_aptos_tracer(explicit_bin: Option<&str>) -> DiscoveryResult {
    if let Some(bin) = explicit_bin {
        if !bin.trim().is_empty() {
            return DiscoveryResult {
                path: Some(PathBuf::from(bin)),
                source: Some("flag:--tracer-bin".to_owned()),
            };
        }
    }

    if let Some(path) = find_in_path(APTOS_TRACER_BIN) {
        return DiscoveryResult {
            path: Some(path),
            source: Some("PATH".to_owned()),
        };
    }

    DiscoveryResult {
        path: None,
        source: None,
    }
}

fn resolve_aptos_script_compose(explicit_bin: Option<&str>) -> DiscoveryResult {
    if let Some(bin) = explicit_bin {
        if !bin.trim().is_empty() {
            return DiscoveryResult {
                path: Some(PathBuf::from(bin)),
                source: Some("flag:--script-compose-bin".to_owned()),
            };
        }
    }

    if let Some(path) = find_in_path(APTOS_SCRIPT_COMPOSE_BIN) {
        return DiscoveryResult {
            path: Some(path),
            source: Some("PATH".to_owned()),
        };
    }

    DiscoveryResult {
        path: None,
        source: None,
    }
}

fn find_in_path(bin: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    for path in env::split_paths(&path_var) {
        let full = path.join(bin);
        if full.is_file() {
            return Some(full);
        }
    }
    None
}

fn is_executable(path: &str) -> bool {
    let path = Path::new(path);
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = path.metadata() {
            return metadata.permissions().mode() & 0o111 != 0;
        }
        false
    }

    #[cfg(not(unix))]
    {
        true
    }
}

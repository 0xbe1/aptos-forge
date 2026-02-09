use anyhow::{anyhow, Context, Result};
use aptly_aptos::AptosClient;
use aptly_plugin::run_move_decompiler;
use clap::{Args, Subcommand};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[derive(Args)]
pub(crate) struct DecompileCommand {
    #[command(subcommand)]
    pub(crate) command: DecompileSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum DecompileSubcommand {
    Raw(DecompileRawArgs),
    Module(DecompileModuleArgs),
    Address(DecompileAddressArgs),
}

#[derive(Args)]
pub(crate) struct DecompileRawArgs {
    #[arg(long = "decompiler-bin")]
    pub(crate) decompiler_bin: Option<String>,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    pub(crate) args: Vec<String>,
}

#[derive(Args)]
pub(crate) struct DecompileModuleArgs {
    pub(crate) address: String,
    pub(crate) module: String,
    #[arg(long = "decompiler-bin")]
    pub(crate) decompiler_bin: Option<String>,
    #[arg(long)]
    pub(crate) out_dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub(crate) keep_bytecode: bool,
    #[arg(long = "ending", default_value = "move")]
    pub(crate) ending: String,
    #[arg(long = "decompiler-arg")]
    pub(crate) decompiler_args: Vec<String>,
}

#[derive(Args)]
pub(crate) struct DecompileAddressArgs {
    pub(crate) address: String,
    #[arg(long = "module")]
    pub(crate) modules: Vec<String>,
    #[arg(long = "decompiler-bin")]
    pub(crate) decompiler_bin: Option<String>,
    #[arg(long)]
    pub(crate) out_dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub(crate) keep_bytecode: bool,
    #[arg(long = "ending", default_value = "move")]
    pub(crate) ending: String,
    #[arg(long = "decompiler-arg")]
    pub(crate) decompiler_args: Vec<String>,
}

pub(crate) fn run_decompile(rpc_url: &str, command: DecompileCommand) -> Result<()> {
    match command.command {
        DecompileSubcommand::Raw(args) => {
            run_move_decompiler(args.decompiler_bin.as_deref(), &args.args)
        }
        DecompileSubcommand::Module(args) => run_decompile_for_modules(
            rpc_url,
            &args.address,
            vec![args.module],
            args.decompiler_bin.as_deref(),
            args.out_dir,
            args.keep_bytecode,
            &args.ending,
            &args.decompiler_args,
        ),
        DecompileSubcommand::Address(args) => {
            let client = AptosClient::new(rpc_url)?;
            let modules = if args.modules.is_empty() {
                fetch_account_module_names(&client, &args.address)?
            } else {
                args.modules
            };

            run_decompile_for_modules(
                rpc_url,
                &args.address,
                modules,
                args.decompiler_bin.as_deref(),
                args.out_dir,
                args.keep_bytecode,
                &args.ending,
                &args.decompiler_args,
            )
        }
    }
}

fn run_decompile_for_modules(
    rpc_url: &str,
    address: &str,
    modules: Vec<String>,
    decompiler_bin: Option<&str>,
    out_dir: Option<PathBuf>,
    keep_bytecode: bool,
    ending: &str,
    decompiler_args: &[String],
) -> Result<()> {
    if modules.is_empty() {
        return Err(anyhow!("no modules provided for decompilation"));
    }

    let client = AptosClient::new(rpc_url)?;
    let output_dir = out_dir.unwrap_or_else(|| default_decompile_output_dir(address));
    fs::create_dir_all(&output_dir).with_context(|| {
        format!(
            "failed to create decompile output directory {}",
            output_dir.display()
        )
    })?;

    let temp_dir = tempdir().context("failed to create temporary bytecode directory")?;
    let bytecode_dir = temp_dir.path().join("bytecode");
    fs::create_dir_all(&bytecode_dir)?;

    let mut mv_files = Vec::new();
    for module in modules {
        let module_name = module.trim().to_owned();
        if module_name.is_empty() {
            continue;
        }

        let bytecode_hex = fetch_module_bytecode(&client, address, &module_name)?;
        let file_stem = sanitize_file_component(&module_name);
        let mv_path = bytecode_dir.join(format!("{file_stem}.mv"));
        write_mv_file(&mv_path, &bytecode_hex)?;
        if keep_bytecode {
            let bytecode_out_dir = output_dir.join("bytecode");
            fs::create_dir_all(&bytecode_out_dir)?;
            let destination = bytecode_out_dir.join(format!("{file_stem}.mv"));
            fs::copy(&mv_path, destination).context("failed to preserve bytecode file")?;
        }
        mv_files.push(mv_path);
    }

    if mv_files.is_empty() {
        return Err(anyhow!("no module bytecode found to decompile"));
    }

    let mut run_args = Vec::new();
    run_args.push("--output-dir".to_owned());
    run_args.push(output_dir.display().to_string());
    if !ending.trim().is_empty() {
        run_args.push("--ending".to_owned());
        run_args.push(ending.to_owned());
    }
    run_args.extend(decompiler_args.iter().cloned());
    run_args.extend(mv_files.iter().map(|path| path.display().to_string()));

    run_move_decompiler(decompiler_bin, &run_args)?;
    eprintln!(
        "Decompiled {} module(s) for {} into {}",
        mv_files.len(),
        address,
        output_dir.display()
    );
    Ok(())
}

fn fetch_account_module_names(client: &AptosClient, address: &str) -> Result<Vec<String>> {
    let value = client.get_json(&format!("/accounts/{address}/modules"))?;
    let modules = value
        .as_array()
        .ok_or_else(|| anyhow!("unexpected module list response format"))?;

    let names: Vec<String> = modules
        .iter()
        .filter_map(|module| {
            module
                .get("abi")
                .and_then(|abi| abi.get("name"))
                .and_then(Value::as_str)
                .map(|name| name.to_owned())
        })
        .collect();

    if names.is_empty() {
        return Err(anyhow!("no modules found at address {address}"));
    }
    Ok(names)
}

fn fetch_module_bytecode(client: &AptosClient, address: &str, module: &str) -> Result<String> {
    let encoded = urlencoding::encode(module);
    let value = client.get_json(&format!("/accounts/{address}/module/{encoded}"))?;
    let bytecode = value
        .get("bytecode")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("module {module} has no bytecode field"))?;
    Ok(bytecode.to_owned())
}

fn write_mv_file(path: &Path, bytecode_hex: &str) -> Result<()> {
    let trimmed = bytecode_hex.strip_prefix("0x").unwrap_or(bytecode_hex);
    let bytes = hex::decode(trimmed).context("failed to decode module bytecode hex")?;
    fs::write(path, bytes)
        .with_context(|| format!("failed to write bytecode file {}", path.display()))?;
    Ok(())
}

fn default_decompile_output_dir(address: &str) -> PathBuf {
    PathBuf::from("decompiled").join(sanitize_file_component(address))
}

fn sanitize_file_component(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }

    if sanitized.is_empty() {
        "output".to_owned()
    } else {
        sanitized
    }
}

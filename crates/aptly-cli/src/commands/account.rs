use anyhow::{anyhow, Context, Result};
use aptly_aptos::AptosClient;
use clap::{Args, Subcommand};
use flate2::read::GzDecoder;
use num_bigint::BigInt;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::str::FromStr;

use crate::commands::common::{
    get_nested_string, parse_u64, shorten_addr, value_to_string, with_optional_ledger_version,
};

const PACKAGE_REGISTRY_TYPE: &str = "0x1::code::PackageRegistry";
const FUNGIBLE_METADATA_TYPE: &str = "0x1::fungible_asset::Metadata";

#[derive(Args)]
#[command(
    after_help = "Examples:\n  aptly account 0x1\n  aptly account resources 0x1\n  aptly account resource 0x1 0x1::coin::CoinInfo<0x1::aptos_coin::AptosCoin>\n  aptly account module 0x1 coin --abi\n  aptly account balance 0x1 0x1::aptos_coin::AptosCoin\n  aptly account txs 0x1 --limit 10\n  aptly account sends 0x1 --limit 50 --pretty\n  aptly account source-code 0x1 chain_id --raw\n\nIf source metadata is unavailable:\n  aptly decompile address <address>\n  aptly decompile module <address> <module_name>"
)]
pub(crate) struct AccountCommand {
    #[command(subcommand)]
    pub(crate) command: Option<AccountSubcommand>,
    /// Account address (`0x...`) when no subcommand is provided.
    #[arg(value_name = "ADDRESS")]
    pub(crate) address: Option<String>,
}

#[derive(Subcommand)]
pub(crate) enum AccountSubcommand {
    #[command(about = "List all Move resources under an account")]
    Resources(AddressArg),
    #[command(about = "Read a Move resource by fully-qualified type")]
    Resource(ResourceArgs),
    #[command(about = "List all Move modules published under an account")]
    Modules(AddressArg),
    #[command(about = "Read a module, its ABI only, or its raw bytecode")]
    Module(ModuleArgs),
    #[command(about = "Read fungible asset balance for an account address")]
    Balance(BalanceArgs),
    #[command(about = "List account transactions (with --limit/--start pagination)")]
    Txs(TxsArgs),
    #[command(about = "Summarize outgoing transfers from account transactions")]
    Sends(SendsArgs),
    #[command(
        name = "source-code",
        about = "Fetch published Move source metadata. If unavailable, use `aptly decompile`.",
        after_help = "Fallback when source metadata is unavailable:\n  aptly decompile address <address>\n  aptly decompile module <address> <module_name>"
    )]
    SourceCode(SourceCodeArgs),
}

#[derive(Args)]
pub(crate) struct AddressArg {
    /// Account address (`0x...`).
    #[arg(value_name = "ADDRESS")]
    pub(crate) address: String,
    /// Read from a historical ledger version.
    #[arg(long)]
    pub(crate) ledger_version: Option<u64>,
}

#[derive(Args)]
pub(crate) struct ResourceArgs {
    /// Account address (`0x...`).
    #[arg(value_name = "ADDRESS")]
    pub(crate) address: String,
    /// Fully-qualified Move resource type.
    #[arg(value_name = "RESOURCE_TYPE")]
    pub(crate) resource_type: String,
    /// Read from a historical ledger version.
    #[arg(long)]
    pub(crate) ledger_version: Option<u64>,
}

#[derive(Args)]
pub(crate) struct ModuleArgs {
    /// Account address (`0x...`).
    #[arg(value_name = "ADDRESS")]
    pub(crate) address: String,
    /// Module name.
    #[arg(value_name = "MODULE_NAME")]
    pub(crate) module_name: String,
    /// Read from a historical ledger version.
    #[arg(long)]
    pub(crate) ledger_version: Option<u64>,
    /// Print only ABI from module response.
    #[arg(long)]
    pub(crate) abi: bool,
    /// Print only bytecode from module response.
    #[arg(long)]
    pub(crate) bytecode: bool,
}

#[derive(Args)]
pub(crate) struct BalanceArgs {
    /// Account address (`0x...`).
    #[arg(value_name = "ADDRESS")]
    pub(crate) address: String,
    /// Optional asset type tag; defaults to AptosCoin.
    #[arg(value_name = "ASSET_TYPE")]
    pub(crate) asset_type: Option<String>,
    /// Read from a historical ledger version.
    #[arg(long)]
    pub(crate) ledger_version: Option<u64>,
}

#[derive(Args)]
pub(crate) struct TxsArgs {
    /// Account address (`0x...`).
    #[arg(value_name = "ADDRESS")]
    pub(crate) address: String,
    /// Maximum number of transactions to return.
    #[arg(long, default_value_t = 25)]
    pub(crate) limit: u64,
    /// Start cursor (ledger version offset).
    #[arg(long, default_value_t = 0)]
    pub(crate) start: u64,
}

#[derive(Args)]
pub(crate) struct SendsArgs {
    /// Account address (`0x...`).
    #[arg(value_name = "ADDRESS")]
    pub(crate) address: String,
    /// Maximum number of transactions to scan.
    #[arg(long, default_value_t = 25)]
    pub(crate) limit: u64,
    /// Render human-friendly decimal amounts and symbols.
    #[arg(long, default_value_t = false)]
    pub(crate) pretty: bool,
}

#[derive(Args)]
pub(crate) struct SourceCodeArgs {
    /// Account address (`0x...`).
    #[arg(value_name = "ADDRESS")]
    pub(crate) address: String,
    /// Optional module name filter.
    #[arg(value_name = "MODULE_NAME")]
    pub(crate) module_name: Option<String>,
    /// Optional package name filter.
    #[arg(long = "package")]
    pub(crate) package_name: Option<String>,
    /// Read from a historical ledger version.
    #[arg(long)]
    pub(crate) ledger_version: Option<u64>,
    /// Print raw package/module/source JSON.
    #[arg(long, default_value_t = false)]
    pub(crate) raw: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ModuleSource {
    package: String,
    module: String,
    source: String,
}

#[derive(Debug, Clone, Serialize)]
struct Transfer {
    from: String,
    to: String,
    amount: String,
    asset: String,
    version: u64,
}

#[derive(Debug, Clone, Default)]
struct AssetMetadata {
    symbol: String,
    decimals: u8,
}

pub(crate) fn run_account(client: &AptosClient, command: AccountCommand) -> Result<()> {
    match (command.command, command.address) {
        (Some(AccountSubcommand::Resources(args)), _) => {
            let path = with_optional_ledger_version(
                &format!("/accounts/{}/resources", args.address),
                args.ledger_version,
            );
            let value = client.get_json(&path)?;
            crate::print_pretty_json(&value)
        }
        (Some(AccountSubcommand::Resource(args)), _) => {
            let encoded = urlencoding::encode(&args.resource_type);
            let path = with_optional_ledger_version(
                &format!("/accounts/{}/resource/{encoded}", args.address),
                args.ledger_version,
            );
            let value = client.get_json(&path)?;
            crate::print_pretty_json(&value)
        }
        (Some(AccountSubcommand::Modules(args)), _) => {
            let path = with_optional_ledger_version(
                &format!("/accounts/{}/modules", args.address),
                args.ledger_version,
            );
            let value = client.get_json(&path)?;
            crate::print_pretty_json(&value)
        }
        (Some(AccountSubcommand::Module(args)), _) => {
            let path = with_optional_ledger_version(
                &format!("/accounts/{}/module/{}", args.address, args.module_name),
                args.ledger_version,
            );
            let value = client.get_json(&path)?;

            if !args.abi && !args.bytecode {
                return crate::print_pretty_json(&value);
            }

            if args.abi {
                let abi = value.get("abi").cloned().unwrap_or(Value::Null);
                return crate::print_pretty_json(&abi);
            }

            let bytecode = value.get("bytecode").cloned().unwrap_or(Value::Null);
            crate::print_pretty_json(&bytecode)
        }
        (Some(AccountSubcommand::Balance(args)), _) => {
            let asset_type = args
                .asset_type
                .unwrap_or_else(|| "0x1::aptos_coin::AptosCoin".to_owned());
            let encoded = urlencoding::encode(&asset_type);
            let path = with_optional_ledger_version(
                &format!("/accounts/{}/balance/{encoded}", args.address),
                args.ledger_version,
            );
            let value = client.get_json(&path)?;
            crate::print_pretty_json(&value)
        }
        (Some(AccountSubcommand::Txs(args)), _) => {
            let mut path = format!(
                "/accounts/{}/transactions?limit={}",
                args.address, args.limit
            );
            if args.start > 0 {
                path.push_str(&format!("&start={}", args.start));
            }
            let value = client.get_json(&path)?;
            crate::print_pretty_json(&value)
        }
        (Some(AccountSubcommand::Sends(args)), _) => run_account_sends(client, &args),
        (Some(AccountSubcommand::SourceCode(args)), _) => run_account_source_code(client, &args),
        (None, Some(address)) => {
            let value = client.get_json(&format!("/accounts/{address}"))?;
            crate::print_pretty_json(&value)
        }
        (None, None) => Err(anyhow!("missing address or subcommand")),
    }
}

fn run_account_source_code(client: &AptosClient, args: &SourceCodeArgs) -> Result<()> {
    let resource_type = urlencoding::encode(PACKAGE_REGISTRY_TYPE);
    let path = with_optional_ledger_version(
        &format!("/accounts/{}/resource/{resource_type}", args.address),
        args.ledger_version,
    );

    let resource = match client.get_json(&path) {
        Ok(data) => data,
        Err(err) => {
            let message = err.to_string();
            if message.contains("resource_not_found") || message.contains("status 404") {
                return Err(anyhow!(
                    "no code metadata found at address; use `aptly decompile address {}`",
                    args.address
                ));
            }
            return Err(err);
        }
    };

    let package_filter = args.package_name.as_deref();
    let module_filter = args.module_name.as_deref();
    let packages = resource
        .get("data")
        .and_then(|v| v.get("packages"))
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("failed to parse package registry resource"))?;

    let mut sources = Vec::new();
    let mut module_exists = false;

    for package in packages {
        let package_name = package
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if let Some(filter) = package_filter {
            if package_name != filter {
                continue;
            }
        }

        let Some(modules) = package.get("modules").and_then(Value::as_array) else {
            continue;
        };

        for module in modules {
            let module_name = module
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();

            if let Some(filter) = module_filter {
                if module_name == filter {
                    module_exists = true;
                } else {
                    continue;
                }
            }

            let Some(source_hex) = module.get("source").and_then(Value::as_str) else {
                continue;
            };
            if source_hex.is_empty() {
                continue;
            }

            if let Ok(source) = decode_source(source_hex) {
                sources.push(ModuleSource {
                    package: package_name.clone(),
                    module: module_name,
                    source,
                });
            }
        }
    }

    if sources.is_empty() {
        if let Some(module_name) = module_filter {
            if module_exists {
                return Err(anyhow!(
                    "no source code available (compiled without --save-metadata); use `aptly decompile module {} {}`",
                    args.address,
                    module_name
                ));
            }
            return Err(anyhow!("module {module_name:?} not found"));
        }
        return Err(anyhow!(
            "no source code available (compiled without --save-metadata); use `aptly decompile address {}`",
            args.address
        ));
    }

    if args.raw {
        if sources.len() != 1 {
            return Err(anyhow!(
                "--raw requires exactly one module match (found {})",
                sources.len()
            ));
        }
        print!("{}", sources[0].source);
        return Ok(());
    }

    crate::print_serialized(&sources)
}

fn decode_source(hex_source: &str) -> Result<String> {
    let trimmed = hex_source.strip_prefix("0x").unwrap_or(hex_source);
    let gzipped = hex::decode(trimmed).context("failed to decode source hex")?;
    let mut decoder = GzDecoder::new(gzipped.as_slice());
    let mut output = String::new();
    decoder
        .read_to_string(&mut output)
        .context("failed to decompress source")?;
    Ok(output)
}

fn run_account_sends(client: &AptosClient, args: &SendsArgs) -> Result<()> {
    let path = format!(
        "/accounts/{}/transactions?limit={}",
        args.address, args.limit
    );
    let txs = client.get_json(&path)?;
    let tx_array = txs
        .as_array()
        .ok_or_else(|| anyhow!("unexpected transactions response format"))?;

    let mut metadata_cache: HashMap<String, AssetMetadata> = HashMap::new();
    let mut transfers = Vec::new();

    for tx in tx_array {
        if let Some(transfer) = extract_transfer(client, tx, &mut metadata_cache) {
            transfers.push(transfer);
        }
    }

    if args.pretty {
        print_pretty_sends(&transfers);
        return Ok(());
    }

    crate::print_serialized(&transfers)
}

fn extract_transfer(
    client: &AptosClient,
    tx: &Value,
    metadata_cache: &mut HashMap<String, AssetMetadata>,
) -> Option<Transfer> {
    if tx.get("type")?.as_str()? != "user_transaction" {
        return None;
    }

    let payload = tx.get("payload")?;
    if payload.get("type")?.as_str()? != "entry_function_payload" {
        return None;
    }

    let function = payload.get("function")?.as_str()?;
    let args = payload.get("arguments")?.as_array()?;
    let type_args: Vec<String> = payload
        .get("type_arguments")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_owned()))
                .collect()
        })
        .unwrap_or_default();

    let (to, amount_str, asset, is_fungible_asset) = match function {
        "0x1::aptos_account::transfer_coins" | "0x1::coin::transfer" => {
            if args.len() < 2 || type_args.is_empty() {
                return None;
            }
            (
                value_to_string(&args[0]),
                value_to_string(&args[1]),
                type_args[0].clone(),
                false,
            )
        }
        "0x1::primary_fungible_store::transfer" => {
            if args.len() < 3 {
                return None;
            }
            (
                value_to_string(&args[1]),
                value_to_string(&args[2]),
                get_inner_or_string(&args[0]),
                true,
            )
        }
        _ => return None,
    };

    if to.is_empty() || amount_str.is_empty() || asset.is_empty() {
        return None;
    }

    let metadata = get_asset_metadata(client, metadata_cache, &asset, is_fungible_asset);
    let sender = tx
        .get("sender")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let version = parse_u64(tx.get("version").unwrap_or(&Value::Null)).unwrap_or(0);

    Some(Transfer {
        from: sender,
        to,
        amount: format_amount(&amount_str, metadata.decimals),
        asset: metadata.symbol,
        version,
    })
}

fn get_asset_metadata(
    client: &AptosClient,
    cache: &mut HashMap<String, AssetMetadata>,
    asset: &str,
    is_fungible_asset: bool,
) -> AssetMetadata {
    if let Some(cached) = cache.get(asset) {
        return cached.clone();
    }

    let metadata = if is_fungible_asset {
        query_fungible_asset_metadata(client, asset)
    } else {
        query_coin_metadata(client, asset)
    };
    cache.insert(asset.to_owned(), metadata.clone());
    metadata
}

fn query_fungible_asset_metadata(client: &AptosClient, metadata_addr: &str) -> AssetMetadata {
    let mut metadata = AssetMetadata {
        symbol: shorten_addr(metadata_addr),
        decimals: 0,
    };

    let encoded_resource = urlencoding::encode(FUNGIBLE_METADATA_TYPE);
    let path = format!("/accounts/{metadata_addr}/resource/{encoded_resource}");

    if let Ok(resource) = client.get_json(&path) {
        let symbol = get_nested_string(&resource, &["data", "symbol"]);
        if !symbol.is_empty() {
            metadata.symbol = symbol;
        }

        if let Some(decimals) = parse_u64(
            resource
                .get("data")
                .and_then(|d| d.get("decimals"))
                .unwrap_or(&Value::Null),
        ) {
            metadata.decimals = decimals as u8;
        }
    }

    metadata
}

fn query_coin_metadata(client: &AptosClient, coin_type: &str) -> AssetMetadata {
    if coin_type == "0x1::aptos_coin::AptosCoin" {
        return AssetMetadata {
            symbol: "APT".to_owned(),
            decimals: 8,
        };
    }

    let mut metadata = AssetMetadata {
        symbol: shorten_addr(coin_type),
        decimals: 0,
    };

    let Some(issuer) = coin_type.split("::").next() else {
        return metadata;
    };
    if issuer.is_empty() {
        return metadata;
    }

    let resource_type = format!("0x1::coin::CoinInfo<{coin_type}>");
    let encoded_resource = urlencoding::encode(&resource_type);
    let path = format!("/accounts/{issuer}/resource/{encoded_resource}");

    if let Ok(resource) = client.get_json(&path) {
        let symbol = get_nested_string(&resource, &["data", "symbol"]);
        if !symbol.is_empty() {
            metadata.symbol = symbol;
        }

        if let Some(decimals) = parse_u64(
            resource
                .get("data")
                .and_then(|d| d.get("decimals"))
                .unwrap_or(&Value::Null),
        ) {
            metadata.decimals = decimals as u8;
        }
    }

    metadata
}

fn format_amount(amount: &str, decimals: u8) -> String {
    if decimals == 0 {
        return amount.to_owned();
    }

    let Ok(raw) = BigInt::from_str(amount) else {
        return amount.to_owned();
    };

    let divisor = BigInt::from(10u8).pow(decimals as u32);
    let int_part = &raw / &divisor;
    let frac_part = &raw % &divisor;
    let mut frac_str = format!("{:0width$}", frac_part, width = decimals as usize);
    while frac_str.ends_with('0') {
        frac_str.pop();
    }

    if frac_str.is_empty() {
        int_part.to_string()
    } else {
        format!("{int_part}.{frac_str}")
    }
}

fn print_pretty_sends(transfers: &[Transfer]) {
    let max_amount_len = transfers.iter().map(|t| t.amount.len()).max().unwrap_or(0);
    let max_asset_len = transfers.iter().map(|t| t.asset.len()).max().unwrap_or(0);

    for transfer in transfers {
        println!(
            "[{}] {:>amount_width$} {:<asset_width$} → {}",
            transfer.version,
            transfer.amount,
            transfer.asset,
            transfer.to,
            amount_width = max_amount_len,
            asset_width = max_asset_len
        );
    }
}

fn get_inner_or_string(value: &Value) -> String {
    if let Some(inner) = value.get("inner").and_then(Value::as_str) {
        return inner.to_owned();
    }
    value_to_string(value)
}

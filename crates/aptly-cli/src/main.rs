use anyhow::{anyhow, Context, Result};
use aptly_aptos::AptosClient;
use aptly_core::{print_pretty_json, DEFAULT_RPC_URL};
use clap::{Args, Parser, Subcommand};
use flate2::read::GzDecoder;
use num_bigint::BigInt;
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, IsTerminal, Read};
use std::str::FromStr;

const LABELS_URL: &str =
    "https://raw.githubusercontent.com/ThalaLabs/aptos-labels/main/mainnet.json";
const PACKAGE_REGISTRY_TYPE: &str = "0x1::code::PackageRegistry";
const OBJECT_CORE_TYPE: &str = "0x1::object::ObjectCore";
const FUNGIBLE_STORE_TYPE: &str = "0x1::fungible_asset::FungibleStore";
const FUNGIBLE_METADATA_TYPE: &str = "0x1::fungible_asset::Metadata";

#[derive(Parser)]
#[command(name = "aptly-rs")]
#[command(about = "Aptos CLI utilities in Rust")]
struct Cli {
    #[arg(long, global = true, default_value = DEFAULT_RPC_URL)]
    rpc_url: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Node(NodeCommand),
    Account(AccountCommand),
    Address(AddressCommand),
    Block(BlockCommand),
    Events(EventsCommand),
    Table(TableCommand),
    View(ViewCommand),
    Tx(TxCommand),
    Version,
}

#[derive(Args)]
struct NodeCommand {
    #[command(subcommand)]
    command: NodeSubcommand,
}

#[derive(Subcommand)]
enum NodeSubcommand {
    Ledger,
    Spec,
    Health,
    Info,
    #[command(name = "estimate-gas-price")]
    EstimateGasPrice,
}

#[derive(Args)]
struct AccountCommand {
    #[command(subcommand)]
    command: Option<AccountSubcommand>,
    address: Option<String>,
}

#[derive(Subcommand)]
enum AccountSubcommand {
    Resources(AddressArg),
    Resource(ResourceArgs),
    Modules(AddressArg),
    Module(ModuleArgs),
    Balance(BalanceArgs),
    Txs(TxsArgs),
    Sends(SendsArgs),
    #[command(name = "source-code")]
    SourceCode(SourceCodeArgs),
}

#[derive(Args)]
struct AddressArg {
    address: String,
}

#[derive(Args)]
struct ResourceArgs {
    address: String,
    resource_type: String,
}

#[derive(Args)]
struct ModuleArgs {
    address: String,
    module_name: String,
    #[arg(long)]
    abi: bool,
    #[arg(long)]
    bytecode: bool,
}

#[derive(Args)]
struct BalanceArgs {
    address: String,
    asset_type: Option<String>,
}

#[derive(Args)]
struct TxsArgs {
    address: String,
    #[arg(long, default_value_t = 25)]
    limit: u64,
    #[arg(long, default_value_t = 0)]
    start: u64,
}

#[derive(Args)]
struct SendsArgs {
    address: String,
    #[arg(long, default_value_t = 25)]
    limit: u64,
    #[arg(long, default_value_t = false)]
    pretty: bool,
}

#[derive(Args)]
struct SourceCodeArgs {
    address: String,
    module_name: Option<String>,
    #[arg(long = "package")]
    package_name: Option<String>,
    #[arg(long, default_value_t = false)]
    raw: bool,
}

#[derive(Args)]
struct AddressCommand {
    query: String,
}

#[derive(Args)]
struct BlockCommand {
    #[command(subcommand)]
    command: Option<BlockSubcommand>,
    height: Option<String>,
    #[arg(long, default_value_t = false)]
    with_transactions: bool,
}

#[derive(Subcommand)]
enum BlockSubcommand {
    #[command(name = "by-version")]
    ByVersion(ByVersionArgs),
}

#[derive(Args)]
struct ByVersionArgs {
    version: String,
    #[arg(long, default_value_t = false)]
    with_transactions: bool,
}

#[derive(Args)]
struct EventsCommand {
    address: String,
    creation_number: String,
    #[arg(long, default_value_t = 25)]
    limit: u64,
    #[arg(long, default_value_t = 0)]
    start: u64,
}

#[derive(Args)]
struct TableCommand {
    #[command(subcommand)]
    command: TableSubcommand,
}

#[derive(Subcommand)]
enum TableSubcommand {
    Item(TableItemArgs),
}

#[derive(Args)]
struct TableItemArgs {
    table_handle: String,
    #[arg(long)]
    key_type: String,
    #[arg(long)]
    value_type: String,
    #[arg(long)]
    key: String,
}

#[derive(Args)]
struct ViewCommand {
    function: String,
    #[arg(long = "type-args")]
    type_args: Vec<String>,
    #[arg(long = "args")]
    args: Vec<String>,
}

#[derive(Args)]
struct TxCommand {
    #[command(subcommand)]
    command: Option<TxSubcommand>,
    version_or_hash: Option<String>,
}

#[derive(Subcommand)]
enum TxSubcommand {
    List(TxListArgs),
    Submit,
    #[command(name = "balance-change")]
    BalanceChange(TxBalanceChangeArgs),
}

#[derive(Args)]
struct TxListArgs {
    #[arg(long, default_value_t = 25)]
    limit: u64,
    #[arg(long, default_value_t = 0)]
    start: u64,
}

#[derive(Args)]
struct TxBalanceChangeArgs {
    version_or_hash: Option<String>,
    #[arg(long, default_value_t = false)]
    aggregate: bool,
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

#[derive(Debug, Clone, Serialize)]
struct BalanceChange {
    #[serde(rename = "type")]
    event_type: String,
    account: String,
    fungible_store: String,
    asset: String,
    amount: String,
}

#[derive(Debug, Clone, Serialize)]
struct AggregatedBalanceChange {
    account: String,
    asset: String,
    amount: String,
}

#[derive(Debug, Clone, Default)]
struct AssetMetadata {
    symbol: String,
    decimals: u8,
}

#[derive(Debug, Clone, Default)]
struct TransferStoreMetadata {
    owner: String,
    asset: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Command::Version = cli.command {
        print_version();
        return Ok(());
    }

    let client = AptosClient::new(&cli.rpc_url)?;
    match cli.command {
        Command::Node(command) => run_node(&client, command)?,
        Command::Account(command) => run_account(&client, command)?,
        Command::Address(command) => run_address(command)?,
        Command::Block(command) => run_block(&client, command)?,
        Command::Events(command) => run_events(&client, command)?,
        Command::Table(command) => run_table(&client, command)?,
        Command::View(command) => run_view(&client, command)?,
        Command::Tx(command) => run_tx(&client, command)?,
        Command::Version => {}
    }

    Ok(())
}

fn print_version() {
    let version = env!("CARGO_PKG_VERSION");
    let commit_sha = option_env!("APTLY_GIT_SHA").unwrap_or("unknown");
    let build_date = option_env!("APTLY_BUILD_DATE").unwrap_or("unknown");

    println!("aptly-rs {version}");
    println!("commit: {commit_sha}");
    println!("built: {build_date}");
}

fn print_serialized<T: Serialize>(value: &T) -> Result<()> {
    let json_value = serde_json::to_value(value)?;
    print_pretty_json(&json_value)
}

fn run_node(client: &AptosClient, command: NodeCommand) -> Result<()> {
    let value = match command.command {
        NodeSubcommand::Ledger => client.get_json("/")?,
        NodeSubcommand::Spec => client.get_json("/spec.json")?,
        NodeSubcommand::Health => client.get_json("/-/healthy")?,
        NodeSubcommand::Info => client.get_json("/info")?,
        NodeSubcommand::EstimateGasPrice => client.get_json("/estimate_gas_price")?,
    };

    print_pretty_json(&value)
}

fn run_account(client: &AptosClient, command: AccountCommand) -> Result<()> {
    match (command.command, command.address) {
        (Some(AccountSubcommand::Resources(args)), _) => {
            let value = client.get_json(&format!("/accounts/{}/resources", args.address))?;
            print_pretty_json(&value)
        }
        (Some(AccountSubcommand::Resource(args)), _) => {
            let encoded = urlencoding::encode(&args.resource_type);
            let value =
                client.get_json(&format!("/accounts/{}/resource/{encoded}", args.address))?;
            print_pretty_json(&value)
        }
        (Some(AccountSubcommand::Modules(args)), _) => {
            let value = client.get_json(&format!("/accounts/{}/modules", args.address))?;
            print_pretty_json(&value)
        }
        (Some(AccountSubcommand::Module(args)), _) => {
            let path = format!("/accounts/{}/module/{}", args.address, args.module_name);
            let value = client.get_json(&path)?;

            if !args.abi && !args.bytecode {
                return print_pretty_json(&value);
            }

            if args.abi {
                let abi = value.get("abi").cloned().unwrap_or(Value::Null);
                return print_pretty_json(&abi);
            }

            let bytecode = value.get("bytecode").cloned().unwrap_or(Value::Null);
            print_pretty_json(&bytecode)
        }
        (Some(AccountSubcommand::Balance(args)), _) => {
            let asset_type = args
                .asset_type
                .unwrap_or_else(|| "0x1::aptos_coin::AptosCoin".to_owned());
            let encoded = urlencoding::encode(&asset_type);
            let value =
                client.get_json(&format!("/accounts/{}/balance/{encoded}", args.address))?;
            print_pretty_json(&value)
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
            print_pretty_json(&value)
        }
        (Some(AccountSubcommand::Sends(args)), _) => run_account_sends(client, &args),
        (Some(AccountSubcommand::SourceCode(args)), _) => run_account_source_code(client, &args),
        (None, Some(address)) => {
            let value = client.get_json(&format!("/accounts/{address}"))?;
            print_pretty_json(&value)
        }
        (None, None) => Err(anyhow!("missing address or subcommand")),
    }
}

fn run_address(command: AddressCommand) -> Result<()> {
    let response =
        reqwest::blocking::get(LABELS_URL).context("failed to fetch address labels source")?;
    let status = response.status();
    let body = response
        .text()
        .context("failed to read labels response body")?;

    if status != StatusCode::OK {
        return Err(anyhow!("API error (status {}): {}", status.as_u16(), body));
    }

    let labels: HashMap<String, String> =
        serde_json::from_str(&body).context("failed to decode labels response")?;

    let query = command.query.to_lowercase();
    let matches: HashMap<String, String> = labels
        .into_iter()
        .filter(|(_, label)| label.to_lowercase().contains(&query))
        .collect();

    print_serialized(&matches)
}

fn run_account_source_code(client: &AptosClient, args: &SourceCodeArgs) -> Result<()> {
    let resource_type = urlencoding::encode(PACKAGE_REGISTRY_TYPE);
    let path = format!("/accounts/{}/resource/{resource_type}", args.address);

    let resource = match client.get_json(&path) {
        Ok(data) => data,
        Err(err) => {
            let message = err.to_string();
            if message.contains("resource_not_found") || message.contains("status 404") {
                return Err(anyhow!("no code found at address"));
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
                    "no source code available (compiled without --save-metadata)"
                ));
            }
            return Err(anyhow!("module {module_name:?} not found"));
        }
        return Err(anyhow!(
            "no source code available (compiled without --save-metadata)"
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

    print_serialized(&sources)
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

    print_serialized(&transfers)
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
    frac_str.truncate(frac_str.len());
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

fn run_block(client: &AptosClient, command: BlockCommand) -> Result<()> {
    match command.command {
        Some(BlockSubcommand::ByVersion(args)) => {
            let path = format!(
                "/blocks/by_version/{}?with_transactions={}",
                args.version, args.with_transactions
            );
            let value = client.get_json(&path)?;
            print_pretty_json(&value)
        }
        None => {
            let height = command
                .height
                .ok_or_else(|| anyhow!("missing block height or subcommand"))?;
            let path = format!(
                "/blocks/by_height/{height}?with_transactions={}",
                command.with_transactions
            );
            let value = client.get_json(&path)?;
            print_pretty_json(&value)
        }
    }
}

fn run_events(client: &AptosClient, command: EventsCommand) -> Result<()> {
    let mut path = format!(
        "/accounts/{}/events/{}?limit={}",
        command.address, command.creation_number, command.limit
    );
    if command.start > 0 {
        path.push_str(&format!("&start={}", command.start));
    }

    let value = client.get_json(&path)?;
    print_pretty_json(&value)
}

fn run_table(client: &AptosClient, command: TableCommand) -> Result<()> {
    match command.command {
        TableSubcommand::Item(args) => {
            let key_value: Value = serde_json::from_str(&args.key)
                .with_context(|| format!("failed to parse key as JSON: {}", args.key))?;

            let body = json!({
                "key_type": args.key_type,
                "value_type": args.value_type,
                "key": key_value
            });

            let value = client.post_json(&format!("/tables/{}/item", args.table_handle), &body)?;
            print_pretty_json(&value)
        }
    }
}

fn run_view(client: &AptosClient, command: ViewCommand) -> Result<()> {
    let mut parsed_args = Vec::with_capacity(command.args.len());
    for argument in &command.args {
        let parsed: Value = serde_json::from_str(argument)
            .with_context(|| format!("failed to parse argument {argument:?} as JSON"))?;
        parsed_args.push(parsed);
    }

    let body = json!({
        "function": command.function,
        "type_arguments": command.type_args,
        "arguments": parsed_args
    });

    let value = client.post_json("/view", &body)?;
    print_pretty_json(&value)
}

fn run_tx(client: &AptosClient, command: TxCommand) -> Result<()> {
    match (command.command, command.version_or_hash) {
        (Some(TxSubcommand::List(args)), _) => {
            let mut path = format!("/transactions?limit={}", args.limit);
            if args.start > 0 {
                path.push_str(&format!("&start={}", args.start));
            }
            let value = client.get_json(&path)?;
            print_pretty_json(&value)
        }
        (Some(TxSubcommand::Submit), _) => {
            let reader = io::stdin();
            let txn: Value = serde_json::from_reader(reader.lock())
                .context("failed to parse signed transaction JSON from stdin")?;
            let value = client.post_json("/transactions", &txn)?;
            print_pretty_json(&value)
        }
        (Some(TxSubcommand::BalanceChange(args)), _) => run_tx_balance_change(client, &args),
        (None, Some(version_or_hash)) => {
            let path = if version_or_hash.parse::<u64>().is_ok() {
                format!("/transactions/by_version/{version_or_hash}")
            } else {
                format!("/transactions/by_hash/{version_or_hash}")
            };
            let value = client.get_json(&path)?;
            print_pretty_json(&value)
        }
        (None, None) => Err(anyhow!("missing version/hash or subcommand")),
    }
}

fn run_tx_balance_change(client: &AptosClient, args: &TxBalanceChangeArgs) -> Result<()> {
    let tx = get_transaction(client, args.version_or_hash.as_deref())?;
    if tx.get("type").and_then(Value::as_str).unwrap_or_default() != "user_transaction" {
        return Err(anyhow!("not a user transaction"));
    }

    let version = parse_u64(tx.get("version").unwrap_or(&Value::Null)).unwrap_or(0);
    let mut store_info = extract_transfer_store_info_from_tx(&tx);
    let events = build_balance_change_events(&tx, &mut store_info, client, version);

    if args.aggregate {
        let aggregated = aggregate_events(&events);
        return print_serialized(&aggregated);
    }

    print_serialized(&events)
}

fn get_transaction(client: &AptosClient, version_or_hash: Option<&str>) -> Result<Value> {
    if !io::stdin().is_terminal() {
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .context("failed to read transaction from stdin")?;
        if !input.trim().is_empty() {
            let tx: Value =
                serde_json::from_str(&input).context("failed to parse transaction JSON")?;
            return Ok(tx);
        }
    }

    let tx_ref = version_or_hash.ok_or_else(|| anyhow!("no transaction provided"))?;
    if tx_ref.parse::<u64>().is_ok() {
        return client.get_json(&format!("/transactions/by_version/{tx_ref}"));
    }

    client.get_json(&format!("/transactions/by_hash/{tx_ref}"))
}

fn build_balance_change_events(
    tx: &Value,
    store_info: &mut HashMap<String, TransferStoreMetadata>,
    client: &AptosClient,
    version: u64,
) -> Vec<BalanceChange> {
    let mut events = Vec::new();

    let gas_used = parse_bigint(tx.get("gas_used").unwrap_or(&Value::Null));
    let gas_unit_price = parse_bigint(tx.get("gas_unit_price").unwrap_or(&Value::Null));
    let gas_fee = gas_used * gas_unit_price;
    if gas_fee > BigInt::from(0) {
        let sender = tx
            .get("sender")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let apt_store = find_sender_apt_store(tx, &sender);
        events.push(BalanceChange {
            event_type: "gas_fee".to_owned(),
            account: sender,
            fungible_store: apt_store,
            asset: "0xa".to_owned(),
            amount: gas_fee.to_string(),
        });
    }

    let Some(tx_events) = tx.get("events").and_then(Value::as_array) else {
        return events;
    };

    for event in tx_events {
        let Some(event_type) = event.get("type").and_then(Value::as_str) else {
            continue;
        };
        let normalized = match event_type {
            "0x1::fungible_asset::Withdraw" => "withdraw",
            "0x1::fungible_asset::Deposit" => "deposit",
            _ => continue,
        };

        let store = get_nested_string(event, &["data", "store"]);
        let amount = get_nested_string(event, &["data", "amount"]);
        if store.is_empty() || amount.is_empty() {
            continue;
        }

        if !store_info.contains_key(&store) {
            let metadata = query_transfer_store_info(client, &store, version);
            store_info.insert(store.clone(), metadata);
        }
        let metadata = store_info.get(&store).cloned().unwrap_or_default();

        events.push(BalanceChange {
            event_type: normalized.to_owned(),
            account: metadata.owner,
            fungible_store: store,
            asset: metadata.asset,
            amount,
        });
    }

    events
}

fn extract_transfer_store_info_from_tx(tx: &Value) -> HashMap<String, TransferStoreMetadata> {
    let mut owners: HashMap<String, String> = HashMap::new();
    let mut info: HashMap<String, TransferStoreMetadata> = HashMap::new();

    let Some(changes) = tx.get("changes").and_then(Value::as_array) else {
        return info;
    };

    for change in changes {
        if change.get("type").and_then(Value::as_str) != Some("write_resource") {
            continue;
        }
        if change
            .get("data")
            .and_then(|d| d.get("type"))
            .and_then(Value::as_str)
            != Some(OBJECT_CORE_TYPE)
        {
            continue;
        }

        let address = change
            .get("address")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let owner = get_nested_string(change, &["data", "data", "owner"]);
        if !address.is_empty() {
            owners.insert(address, owner);
        }
    }

    for change in changes {
        if change.get("type").and_then(Value::as_str) != Some("write_resource") {
            continue;
        }
        let data_type = change
            .get("data")
            .and_then(|d| d.get("type"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !data_type.contains("fungible_asset::FungibleStore") {
            continue;
        }

        let address = change
            .get("address")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let asset = get_nested_string(change, &["data", "data", "metadata", "inner"]);
        if address.is_empty() {
            continue;
        }

        info.insert(
            address.clone(),
            TransferStoreMetadata {
                owner: owners.get(&address).cloned().unwrap_or_default(),
                asset,
            },
        );
    }

    info
}

fn find_sender_apt_store(tx: &Value, sender: &str) -> String {
    let Some(changes) = tx.get("changes").and_then(Value::as_array) else {
        return String::new();
    };

    let mut owners: HashMap<String, String> = HashMap::new();
    for change in changes {
        if change.get("type").and_then(Value::as_str) != Some("write_resource") {
            continue;
        }
        if change
            .get("data")
            .and_then(|d| d.get("type"))
            .and_then(Value::as_str)
            != Some(OBJECT_CORE_TYPE)
        {
            continue;
        }
        let address = change
            .get("address")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let owner = get_nested_string(change, &["data", "data", "owner"]);
        owners.insert(address, owner);
    }

    for change in changes {
        if change.get("type").and_then(Value::as_str) != Some("write_resource") {
            continue;
        }
        let data_type = change
            .get("data")
            .and_then(|d| d.get("type"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !data_type.contains("fungible_asset::FungibleStore") {
            continue;
        }

        let address = change
            .get("address")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let asset = get_nested_string(change, &["data", "data", "metadata", "inner"]);
        if owners.get(&address).map(String::as_str) == Some(sender) && asset == "0xa" {
            return address;
        }
    }

    String::new()
}

fn query_transfer_store_info(
    client: &AptosClient,
    store: &str,
    version: u64,
) -> TransferStoreMetadata {
    let mut metadata = TransferStoreMetadata::default();
    if store.is_empty() {
        return metadata;
    }

    let mut query = String::new();
    if version > 0 {
        query = format!("?ledger_version={version}");
    }

    let object_type = urlencoding::encode(OBJECT_CORE_TYPE);
    let object_path = format!("/accounts/{store}/resource/{object_type}{query}");
    if let Ok(value) = client.get_json(&object_path) {
        metadata.owner = get_nested_string(&value, &["data", "owner"]);
    }

    let store_type = urlencoding::encode(FUNGIBLE_STORE_TYPE);
    let store_path = format!("/accounts/{store}/resource/{store_type}{query}");
    if let Ok(value) = client.get_json(&store_path) {
        metadata.asset = get_nested_string(&value, &["data", "metadata", "inner"]);
    }

    metadata
}

fn aggregate_events(events: &[BalanceChange]) -> Vec<AggregatedBalanceChange> {
    let mut totals: HashMap<(String, String), BigInt> = HashMap::new();
    let mut order: Vec<(String, String)> = Vec::new();

    for event in events {
        let key = (event.account.clone(), event.asset.clone());
        if !totals.contains_key(&key) {
            totals.insert(key.clone(), BigInt::from(0));
            order.push(key.clone());
        }

        let amount = BigInt::from_str(&event.amount).unwrap_or_else(|_| BigInt::from(0));
        if let Some(total) = totals.get_mut(&key) {
            match event.event_type.as_str() {
                "withdraw" | "gas_fee" => *total -= amount,
                "deposit" => *total += amount,
                _ => {}
            }
        }
    }

    order
        .into_iter()
        .map(|(account, asset)| AggregatedBalanceChange {
            amount: totals
                .get(&(account.clone(), asset.clone()))
                .map(ToString::to_string)
                .unwrap_or_else(|| "0".to_owned()),
            account,
            asset,
        })
        .collect()
}

fn parse_u64(value: &Value) -> Option<u64> {
    match value {
        Value::String(s) => s.parse::<u64>().ok(),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

fn parse_bigint(value: &Value) -> BigInt {
    let string_value = value_to_string(value);
    BigInt::from_str(&string_value).unwrap_or_else(|_| BigInt::from(0))
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn get_inner_or_string(value: &Value) -> String {
    if let Some(inner) = value.get("inner").and_then(Value::as_str) {
        return inner.to_owned();
    }
    value_to_string(value)
}

fn get_nested_string(value: &Value, keys: &[&str]) -> String {
    let mut current = value;
    for key in keys {
        let Some(next) = current.get(*key) else {
            return String::new();
        };
        current = next;
    }
    value_to_string(current)
}

fn shorten_addr(value: &str) -> String {
    if value.len() > 12 {
        format!("{}...{}", &value[..6], &value[value.len() - 4..])
    } else {
        value.to_owned()
    }
}

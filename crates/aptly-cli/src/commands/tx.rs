use anyhow::{anyhow, Context, Result};
use aptly_aptos::AptosClient;
use aptly_plugin::resolve_aptos_tracer_bin;
use clap::{Args, Subcommand};
use num_bigint::BigInt;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, IsTerminal, Read};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::commands::common::{get_nested_string, parse_u64, value_to_string};

const OBJECT_CORE_TYPE: &str = "0x1::object::ObjectCore";
const FUNGIBLE_STORE_TYPE: &str = "0x1::fungible_asset::FungibleStore";
const DEFAULT_TRACER_LISTEN_ADDRESS: &str = "127.0.0.1";
const DEFAULT_TRACER_LISTEN_PORT: u16 = 9201;
const DEFAULT_TRACER_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_TRACER_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Args)]
pub(crate) struct TxCommand {
    #[command(subcommand)]
    pub(crate) command: Option<TxSubcommand>,
    pub(crate) version_or_hash: Option<String>,
}

#[derive(Subcommand)]
pub(crate) enum TxSubcommand {
    List(TxListArgs),
    Encode,
    Simulate(TxSimulateArgs),
    Submit,
    Trace(TxTraceArgs),
    #[command(name = "balance-change")]
    BalanceChange(TxBalanceChangeArgs),
}

#[derive(Args)]
pub(crate) struct TxListArgs {
    #[arg(long, default_value_t = 25)]
    pub(crate) limit: u64,
    #[arg(long, default_value_t = 0)]
    pub(crate) start: u64,
}

#[derive(Args)]
pub(crate) struct TxBalanceChangeArgs {
    pub(crate) version_or_hash: Option<String>,
    #[arg(long, default_value_t = false)]
    pub(crate) aggregate: bool,
}

#[derive(Args)]
pub(crate) struct TxSimulateArgs {
    pub(crate) sender: String,
}

#[derive(Args)]
pub(crate) struct TxTraceArgs {
    pub(crate) version_or_hash: String,
    #[arg(long = "tracer-bin")]
    pub(crate) tracer_bin: Option<String>,
    #[arg(long = "tracer-url")]
    pub(crate) tracer_url: Option<String>,
    #[arg(long = "chain-id")]
    pub(crate) chain_id: Option<u16>,
    #[arg(long = "sentio-endpoint")]
    pub(crate) sentio_endpoint: Option<String>,
    #[arg(long = "listen-address", default_value = DEFAULT_TRACER_LISTEN_ADDRESS)]
    pub(crate) listen_address: String,
    #[arg(long = "listen-port", default_value_t = DEFAULT_TRACER_LISTEN_PORT)]
    pub(crate) listen_port: u16,
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
struct TransferStoreMetadata {
    owner: String,
    asset: String,
}

pub(crate) fn run_tx(client: &AptosClient, rpc_url: &str, command: TxCommand) -> Result<()> {
    match (command.command, command.version_or_hash) {
        (Some(TxSubcommand::List(args)), _) => {
            let mut path = format!("/transactions?limit={}", args.limit);
            if args.start > 0 {
                path.push_str(&format!("&start={}", args.start));
            }
            let value = client.get_json(&path)?;
            crate::print_pretty_json(&value)
        }
        (Some(TxSubcommand::Encode), _) => run_tx_encode(client),
        (Some(TxSubcommand::Simulate(args)), _) => run_tx_simulate(client, &args),
        (Some(TxSubcommand::Trace(args)), _) => run_tx_trace(client, rpc_url, &args),
        (Some(TxSubcommand::Submit), _) => {
            let reader = io::stdin();
            let txn: Value = serde_json::from_reader(reader.lock())
                .context("failed to parse signed transaction JSON from stdin")?;
            let value = client.post_json("/transactions", &txn)?;
            crate::print_pretty_json(&value)
        }
        (Some(TxSubcommand::BalanceChange(args)), _) => run_tx_balance_change(client, &args),
        (None, Some(version_or_hash)) => {
            let path = if version_or_hash.parse::<u64>().is_ok() {
                format!("/transactions/by_version/{version_or_hash}")
            } else {
                format!("/transactions/by_hash/{version_or_hash}")
            };
            let value = client.get_json(&path)?;
            crate::print_pretty_json(&value)
        }
        (None, None) => Err(anyhow!("missing version/hash or subcommand")),
    }
}

fn run_tx_encode(client: &AptosClient) -> Result<()> {
    let reader = io::stdin();
    let txn: Value = serde_json::from_reader(reader.lock())
        .context("failed to parse unsigned transaction JSON from stdin")?;
    let encoded = client.post_json("/transactions/encode_submission", &txn)?;
    crate::print_pretty_json(&encoded)
}

fn run_tx_simulate(client: &AptosClient, args: &TxSimulateArgs) -> Result<()> {
    let stdin_value = read_json_from_stdin("failed to parse payload JSON from stdin")?;
    let payload = normalize_simulation_payload(&stdin_value)?;

    let account = client
        .get_json(&format!("/accounts/{}", args.sender))
        .context("failed to fetch sender account")?;
    let sequence_number = get_nested_string(&account, &["sequence_number"]);
    if sequence_number.is_empty() {
        return Err(anyhow!("failed to resolve sender sequence number"));
    }

    let gas_price = client
        .get_json("/estimate_gas_price")
        .context("failed to fetch gas price estimate")?;
    let gas_unit_price = first_non_empty_string(&[
        get_nested_string(&gas_price, &["gas_estimate"]),
        get_nested_string(&gas_price, &["gas_unit_price"]),
    ])
    .unwrap_or_else(|| "100".to_owned());

    let ledger = client
        .get_json("/")
        .context("failed to fetch ledger info for expiration")?;
    let ledger_timestamp_micros = parse_u64(ledger.get("ledger_timestamp").unwrap_or(&Value::Null))
        .ok_or_else(|| anyhow!("failed to parse ledger timestamp"))?;
    let expiration_timestamp_secs = (ledger_timestamp_micros / 1_000_000 + 600).to_string();

    let simulate_request = json!({
        "sender": args.sender,
        "sequence_number": sequence_number,
        "max_gas_amount": "200000",
        "gas_unit_price": gas_unit_price,
        "expiration_timestamp_secs": expiration_timestamp_secs,
        "payload": payload,
        "signature": {"type": "no_account_signature"}
    });

    let response = client
        .post_json("/transactions/simulate", &simulate_request)
        .context("failed to simulate transaction")?;

    if let Some(first) = response.as_array().and_then(|arr| arr.first()) {
        return crate::print_pretty_json(first);
    }

    crate::print_pretty_json(&response)
}

fn read_json_from_stdin(error_message: &str) -> Result<Value> {
    let reader = io::stdin();
    serde_json::from_reader(reader.lock()).context(error_message.to_owned())
}

fn normalize_simulation_payload(input: &Value) -> Result<Value> {
    if let Some(payload) = input.get("payload") {
        return Ok(payload.clone());
    }

    if input.get("type").is_some() {
        return Ok(input.clone());
    }

    let function = get_nested_string(input, &["function"]);
    if function.is_empty() {
        return Err(anyhow!(
            "payload must contain either `payload`, `type`, or `function` fields"
        ));
    }

    let type_arguments = input
        .get("type_arguments")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let arguments = input
        .get("arguments")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(json!({
        "type": "entry_function_payload",
        "function": function,
        "type_arguments": type_arguments,
        "arguments": arguments
    }))
}

fn run_tx_trace(client: &AptosClient, rpc_url: &str, args: &TxTraceArgs) -> Result<()> {
    let tx_hash = resolve_trace_tx_hash(client, &args.version_or_hash)?;
    let chain_id = resolve_trace_chain_id(client, args.chain_id)?;
    let path = format!("/{chain_id}/call_trace/by_hash/{tx_hash}");

    let mut child = if args.tracer_url.is_some() {
        None
    } else {
        Some(spawn_local_tracer(rpc_url, chain_id, args)?)
    };
    let tracer_base = args
        .tracer_url
        .as_deref()
        .map(|url| url.trim().trim_end_matches('/').to_owned())
        .unwrap_or_else(|| format!("http://{}:{}", args.listen_address.trim(), args.listen_port));

    if child.is_some() {
        wait_for_tracer_ready(args.listen_address.trim(), args.listen_port, &mut child)?;
    }

    let result = fetch_trace_from_tracer(&tracer_base, &path);
    shutdown_local_tracer(&mut child);

    let trace_json = result?;
    match serde_json::from_str::<Value>(&trace_json) {
        Ok(value) => crate::print_pretty_json(&value),
        Err(_) => {
            // Deeply nested traces can exceed serde_json's recursion limit for `Value`.
            // Fall back to raw JSON so tracing still succeeds.
            println!("{trace_json}");
            Ok(())
        }
    }
}

fn resolve_trace_tx_hash(client: &AptosClient, version_or_hash: &str) -> Result<String> {
    let tx_ref = version_or_hash.trim();
    if tx_ref.is_empty() {
        return Err(anyhow!("missing transaction version/hash for trace"));
    }

    if tx_ref.parse::<u64>().is_ok() {
        let tx = client
            .get_json(&format!("/transactions/by_version/{tx_ref}"))
            .context("failed to fetch transaction by version for trace")?;
        let hash = tx
            .get("hash")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("transaction response missing `hash` field"))?;
        return Ok(strip_hex_prefix(hash).to_owned());
    }

    Ok(strip_hex_prefix(tx_ref).to_owned())
}

fn resolve_trace_chain_id(client: &AptosClient, chain_id: Option<u16>) -> Result<u16> {
    if let Some(chain_id) = chain_id {
        return Ok(chain_id);
    }

    let ledger = client
        .get_json("/")
        .context("failed to fetch ledger info for chain id")?;
    let chain_id_u64 = parse_u64(ledger.get("chain_id").unwrap_or(&Value::Null))
        .ok_or_else(|| anyhow!("failed to parse `chain_id` from ledger response"))?;

    u16::try_from(chain_id_u64).context("ledger chain id does not fit in u16")
}

fn spawn_local_tracer(rpc_url: &str, chain_id: u16, args: &TxTraceArgs) -> Result<Child> {
    let tracer_bin = resolve_aptos_tracer_bin(args.tracer_bin.as_deref())?;
    let endpoints = format!("{chain_id}={}", rpc_url.trim());

    let mut command = Command::new(&tracer_bin);
    if let Some(sentio_endpoint) = args.sentio_endpoint.as_deref() {
        if !sentio_endpoint.trim().is_empty() {
            command
                .arg("--sentio-endpoint")
                .arg(sentio_endpoint.trim().to_owned());
        }
    }
    command
        .arg("server-based-on-rest")
        .arg(endpoints)
        .arg(args.listen_address.trim().to_owned())
        .arg(args.listen_port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    command
        .spawn()
        .with_context(|| format!("failed to start aptos-tracer at {}", tracer_bin.display()))
}

fn fetch_trace_from_tracer(base_url: &str, path: &str) -> Result<String> {
    let url = format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let http = reqwest::blocking::Client::builder()
        .timeout(DEFAULT_TRACER_REQUEST_TIMEOUT)
        .build()
        .context("failed to build HTTP client for aptos-tracer")?;

    let response = http
        .get(&url)
        .send()
        .with_context(|| format!("request failed: GET {url}"))?;
    let status = response.status();
    let text = response
        .text()
        .context("failed to read aptos-tracer response body")?;
    if !status.is_success() {
        return Err(anyhow!(
            "aptos-tracer API error (status {}): {}",
            status.as_u16(),
            text
        ));
    }

    Ok(text)
}

fn wait_for_tracer_ready(
    listen_address: &str,
    listen_port: u16,
    child: &mut Option<Child>,
) -> Result<()> {
    let endpoint = format!("{listen_address}:{listen_port}");
    let socket_addrs: Vec<_> = endpoint
        .to_socket_addrs()
        .with_context(|| format!("failed to resolve tracer listen address `{endpoint}`"))?
        .collect();
    if socket_addrs.is_empty() {
        return Err(anyhow!("failed to resolve tracer listen address `{endpoint}`"));
    }

    let deadline = Instant::now() + DEFAULT_TRACER_STARTUP_TIMEOUT;
    loop {
        if let Some(process) = child.as_mut() {
            if let Some(status) = process
                .try_wait()
                .context("failed to inspect aptos-tracer process status")?
            {
                return Err(anyhow!(
                    "aptos-tracer exited before serving requests: {status}"
                ));
            }
        }

        if socket_addrs
            .iter()
            .any(|addr| TcpStream::connect_timeout(addr, Duration::from_millis(200)).is_ok())
        {
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(anyhow!(
                "timed out waiting for aptos-tracer to start after {:?}",
                DEFAULT_TRACER_STARTUP_TIMEOUT
            ));
        }
        sleep(Duration::from_millis(200));
    }
}

fn shutdown_local_tracer(child: &mut Option<Child>) {
    if let Some(process) = child.as_mut() {
        let _ = process.kill();
        let _ = process.wait();
    }
}

fn strip_hex_prefix(value: &str) -> &str {
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value)
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
        return crate::print_serialized(&aggregated);
    }

    crate::print_serialized(&events)
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

fn parse_bigint(value: &Value) -> BigInt {
    let string_value = value_to_string(value);
    BigInt::from_str(&string_value).unwrap_or_else(|_| BigInt::from(0))
}

fn first_non_empty_string(values: &[String]) -> Option<String> {
    values.iter().find(|value| !value.is_empty()).cloned()
}

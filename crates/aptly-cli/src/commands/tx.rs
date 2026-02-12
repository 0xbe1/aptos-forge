use crate::plugin_tools::{resolve_aptos_script_compose_bin, resolve_aptos_tracer_bin};
use anyhow::{anyhow, Context, Result};
use aptly_aptos::AptosClient;
use clap::{Args, Subcommand};
use num_bigint::BigInt;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, IsTerminal, Read};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::time::Duration;

use crate::commands::common::{get_nested_string, parse_u64, value_to_string};

const OBJECT_CORE_TYPE: &str = "0x1::object::ObjectCore";
const FUNGIBLE_STORE_TYPE: &str = "0x1::fungible_asset::FungibleStore";
const DEFAULT_TRACER_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
const SENTIO_TRACE_BASE_URL: &str = "https://app.sentio.xyz";

#[derive(Args)]
pub(crate) struct TxCommand {
    #[command(subcommand)]
    pub(crate) command: Option<TxSubcommand>,
    /// Transaction version (u64) or hash (0x...).
    /// Used when no subcommand is provided.
    pub(crate) version_or_hash: Option<String>,
}

#[derive(Subcommand)]
pub(crate) enum TxSubcommand {
    #[command(about = "List transactions from node API")]
    List(TxListArgs),
    #[command(about = "Encode an unsigned transaction JSON from stdin")]
    Encode,
    #[command(about = "Simulate an entry function payload JSON from stdin")]
    Simulate(TxSimulateArgs),
    #[command(about = "Submit a signed transaction JSON from stdin")]
    Submit,
    #[command(about = "Compose script bytecode from batched call payload JSON on stdin")]
    Compose(TxComposeArgs),
    #[command(about = "Fetch and print transaction call trace")]
    Trace(TxTraceArgs),
    #[command(
        name = "balance-change",
        about = "Summarize fungible asset balance changes for a transaction"
    )]
    BalanceChange(TxBalanceChangeArgs),
}

#[derive(Args)]
pub(crate) struct TxListArgs {
    /// Maximum number of transactions to return.
    #[arg(long, default_value_t = 25)]
    pub(crate) limit: u64,
    /// Start cursor (ledger version offset).
    #[arg(long, default_value_t = 0)]
    pub(crate) start: u64,
}

#[derive(Args)]
pub(crate) struct TxBalanceChangeArgs {
    /// Transaction version (u64) or hash (0x...).
    /// If omitted, reads full transaction JSON from stdin.
    pub(crate) version_or_hash: Option<String>,
    /// Aggregate deltas by `(account, asset)` pair.
    #[arg(long, default_value_t = false)]
    pub(crate) aggregate: bool,
}

#[derive(Args)]
pub(crate) struct TxSimulateArgs {
    /// Sender account address used to resolve sequence number.
    pub(crate) sender: String,
}

#[derive(Args)]
pub(crate) struct TxTraceArgs {
    /// Transaction version (u64) or hash (0x...).
    pub(crate) version_or_hash: String,
    /// Use a local aptos-tracer binary instead of Sentio hosted tracing.
    /// The default hosted mode is usually faster; local mode helps when your
    /// RPC is very fast (for example, your own node).
    #[arg(long = "local-tracer", num_args = 0..=1, value_name = "TRACER_BIN")]
    pub(crate) local_tracer: Option<Option<String>>,
}

#[derive(Args)]
pub(crate) struct TxComposeArgs {
    /// Explicit aptos-script-compose binary path.
    #[arg(long = "script-compose-bin")]
    pub(crate) script_compose_bin: Option<String>,
    /// Keep source metadata in generated script output.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub(crate) with_metadata: bool,
    /// Emit script payload JSON instead of raw 0x-prefixed script bytes.
    #[arg(long, default_value_t = false)]
    pub(crate) emit_script_payload: bool,
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
        (Some(TxSubcommand::Compose(args)), _) => run_tx_compose(rpc_url, &args),
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

fn run_tx_compose(rpc_url: &str, args: &TxComposeArgs) -> Result<()> {
    if io::stdin().is_terminal() {
        return Err(anyhow!(
            "missing compose payload on stdin. Example: `aptly tx compose < payload.json`"
        ));
    }

    let script_compose_bin = resolve_aptos_script_compose_bin(args.script_compose_bin.as_deref())?;

    let mut command = Command::new(&script_compose_bin);
    command
        .arg("--rpc-url")
        .arg(rpc_url.trim())
        .arg("--with-metadata")
        .arg(args.with_metadata.to_string());
    if args.emit_script_payload {
        command.arg("--emit-script-payload");
    }

    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| {
            format!(
                "failed to execute aptos-script-compose at {}",
                script_compose_bin.display()
            )
        })?;
    if !status.success() {
        return Err(anyhow!("aptos-script-compose exited with status {status}"));
    }

    Ok(())
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
    let chain_id = resolve_trace_chain_id(client)?;
    let trace_json = if let Some(local_tracer) = args.local_tracer.as_ref() {
        run_local_trace_with_aptos_tracer(
            rpc_url,
            chain_id,
            &tx_hash,
            local_tracer.as_ref().map(String::as_str),
        )?
    } else {
        fetch_trace_from_external_tracer(chain_id, &tx_hash)?
    };
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

fn resolve_trace_chain_id(client: &AptosClient) -> Result<u16> {
    let ledger = client
        .get_json("/")
        .context("failed to fetch ledger info for trace chain id")?;
    let chain_id_u64 = parse_u64(ledger.get("chain_id").unwrap_or(&Value::Null))
        .ok_or_else(|| anyhow!("failed to parse `chain_id` from ledger response"))?;

    u16::try_from(chain_id_u64).context("ledger chain id does not fit in u16")
}

fn run_local_trace_with_aptos_tracer(
    rpc_url: &str,
    chain_id: u16,
    tx_hash: &str,
    explicit_tracer_bin: Option<&str>,
) -> Result<String> {
    let tracer_bin = resolve_aptos_tracer_bin(explicit_tracer_bin).map_err(|err| {
        anyhow!(
            "{err}\nHint: use `--local-tracer /path/to/aptos-tracer`, or install `aptos-tracer` into PATH."
        )
    })?;
    let mut command = Command::new(&tracer_bin);
    let output = command
        .arg("rest")
        .arg(rpc_url.trim().to_owned())
        .arg(tx_hash.to_owned())
        .arg(chain_id.to_string())
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("failed to execute aptos-tracer at {}", tracer_bin.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let details = if !stderr.trim().is_empty() {
            stderr.trim()
        } else {
            stdout.trim()
        };
        if details.is_empty() {
            return Err(anyhow!("aptos-tracer exited with status {}", output.status));
        }
        return Err(anyhow!(
            "aptos-tracer exited with status {}: {}",
            output.status,
            details
        ));
    }

    let trace_json =
        String::from_utf8(output.stdout).context("aptos-tracer returned non-UTF-8 output")?;
    if trace_json.trim().is_empty() {
        return Err(anyhow!("aptos-tracer returned empty trace output"));
    }

    Ok(trace_json)
}

fn fetch_trace_from_external_tracer(chain_id: u16, tx_hash: &str) -> Result<String> {
    let sentio_url = build_sentio_call_trace_url(chain_id, tx_hash);
    fetch_trace_from_url(&sentio_url)
        .with_context(|| format!("failed to fetch trace from Sentio API `{}`", sentio_url))
}

fn fetch_trace_from_url(url: &str) -> Result<String> {
    let http = reqwest::blocking::Client::builder()
        .timeout(DEFAULT_TRACER_REQUEST_TIMEOUT)
        .build()
        .context("failed to build HTTP client for trace endpoint")?;

    let response = http
        .get(url)
        .send()
        .with_context(|| format!("request failed: GET {url}"))?;
    let status = response.status();
    let text = response
        .text()
        .context("failed to read trace endpoint response body")?;
    if !status.is_success() {
        return Err(anyhow!(
            "trace API error (status {}): {}",
            status.as_u16(),
            text
        ));
    }

    Ok(text)
}

fn build_sentio_call_trace_url(chain_id: u16, tx_hash: &str) -> String {
    format!(
        "{}/api/v1/move/call_trace?networkId={chain_id}&txHash={tx_hash}",
        SENTIO_TRACE_BASE_URL
    )
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

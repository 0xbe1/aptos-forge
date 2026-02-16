use anyhow::Result;
use aptly_aptos::AptosClient;
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::Value;

mod commands;
mod plugin_tools;

use commands::account::{run_account, AccountCommand};
use commands::address::{run_address, AddressCommand};
use commands::block::{run_block, BlockCommand};
use commands::decompile::{run_decompile, DecompileCommand};
use commands::events::{run_events, EventsCommand};
use commands::node::{run_node, NodeCommand};
use commands::plugin::{run_plugin, PluginCommand};
use commands::table::{run_table, TableCommand};
use commands::tx::{run_tx, TxCommand};
use commands::view::{run_view, ViewCommand};

const DEFAULT_RPC_URL: &str = "https://rpc.sentio.xyz/aptos/v1";

#[derive(Parser)]
#[command(name = "aptly")]
#[command(about = "Aptos CLI utilities in Rust")]
struct Cli {
    /// Aptos node REST API endpoint.
    #[arg(long, global = true, default_value = DEFAULT_RPC_URL)]
    rpc_url: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(
        about = "Inspect node and ledger endpoints",
        long_about = "Inspect Aptos node status and metadata. Use subcommands to fetch ledger state, OpenAPI spec, node health, build info, and gas price estimates."
    )]
    Node(NodeCommand),
    #[command(
        about = "Inspect account state (resources, modules, balances, and transactions)",
        long_about = "Inspect account state and activity on Aptos. Use subcommands to query resources, modules, balances, transactions, transfer summaries, and published Move source metadata."
    )]
    Account(AccountCommand),
    #[command(
        about = "Resolve known protocol labels to addresses",
        long_about = "Resolve protocol and ecosystem labels to on-chain addresses using a curated label source."
    )]
    Address(AddressCommand),
    #[command(
        about = "Inspect optional external plugins",
        long_about = "Inspect optional binaries (`move-decompiler`, `aptos-tracer`, `aptos-script-compose`) used by decompile/trace/compose workflows."
    )]
    Plugin(PluginCommand),
    #[command(
        about = "Decompile Move bytecode when source is unavailable",
        long_about = "Decompile Move module bytecode when published source metadata is unavailable from `aptly account source-code`."
    )]
    Decompile(DecompileCommand),
    #[command(
        about = "Fetch blocks by height or version",
        long_about = "Fetch block data either by block height or by a containing ledger version."
    )]
    Block(BlockCommand),
    #[command(
        about = "Read events by account creation number",
        long_about = "Read account events using the account address and event handle creation number, with pagination support."
    )]
    Events(EventsCommand),
    #[command(
        about = "Read Move table items",
        long_about = "Read Move table entries by table handle and typed key/value descriptors."
    )]
    Table(TableCommand),
    #[command(
        about = "Execute view functions",
        long_about = "Execute read-only Move view functions with type arguments, JSON arguments, and optional historical ledger version."
    )]
    View(ViewCommand),
    #[command(
        about = "Inspect, encode, submit, simulate, compose, and trace transactions",
        long_about = "Inspect transactions by version/hash, list transactions, encode or submit payloads via stdin, simulate entry functions, compose scripts, fetch traces, and summarize balance changes."
    )]
    Tx(TxCommand),
    #[command(about = "Print build version information")]
    Version,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let rpc_url = cli.rpc_url.clone();

    match cli.command {
        Command::Version => print_version(),
        Command::Plugin(command) => run_plugin(command)?,
        Command::Decompile(command) => run_decompile(&rpc_url, command)?,
        command => {
            let client = AptosClient::new(&rpc_url)?;
            match command {
                Command::Node(command) => run_node(&client, command)?,
                Command::Account(command) => run_account(&client, command)?,
                Command::Address(command) => run_address(command)?,
                Command::Block(command) => run_block(&client, command)?,
                Command::Events(command) => run_events(&client, command)?,
                Command::Table(command) => run_table(&client, command)?,
                Command::View(command) => run_view(&client, command)?,
                Command::Tx(command) => run_tx(&client, &rpc_url, command)?,
                Command::Plugin(_) | Command::Decompile(_) | Command::Version => unreachable!(),
            }
        }
    }

    Ok(())
}

fn print_version() {
    let version = env!("APTLY_VERSION");
    let commit_sha = env!("APTLY_GIT_SHA");
    let build_date = env!("APTLY_BUILD_DATE");

    println!("aptly {version}");
    println!("commit: {commit_sha}");
    println!("built: {build_date}");
}

pub(crate) fn print_pretty_json(value: &Value) -> Result<()> {
    let rendered = serde_json::to_string_pretty(value)?;
    println!("{rendered}");
    Ok(())
}

pub(crate) fn print_serialized<T: Serialize>(value: &T) -> Result<()> {
    let json_value = serde_json::to_value(value)?;
    print_pretty_json(&json_value)
}

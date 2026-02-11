use anyhow::Result;
use aptly_aptos::AptosClient;
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::Value;

mod commands;

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
    Plugin(PluginCommand),
    Decompile(DecompileCommand),
    Block(BlockCommand),
    Events(EventsCommand),
    Table(TableCommand),
    View(ViewCommand),
    Tx(TxCommand),
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

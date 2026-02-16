use anyhow::{Context, Result};
use aptly_aptos::AptosClient;
use clap::{Args, Subcommand};
use serde_json::{json, Value};

#[derive(Args)]
#[command(
    after_help = "Examples:\n  aptly table item <table_handle> --key-type address --value-type u64 --key '\"0x1\"'\n  aptly table item <table_handle> --key-type u64 --value-type 0x1::coin::CoinInfo<0x1::aptos_coin::AptosCoin> --key '1'"
)]
pub(crate) struct TableCommand {
    #[command(subcommand)]
    pub(crate) command: TableSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum TableSubcommand {
    #[command(about = "Read a table item by key")]
    Item(TableItemArgs),
}

#[derive(Args)]
pub(crate) struct TableItemArgs {
    /// On-chain table handle (`0x...`).
    #[arg(value_name = "TABLE_HANDLE")]
    pub(crate) table_handle: String,
    /// Move type tag for the table key.
    #[arg(long)]
    pub(crate) key_type: String,
    /// Move type tag for the table value.
    #[arg(long)]
    pub(crate) value_type: String,
    /// JSON-encoded key value.
    #[arg(long)]
    pub(crate) key: String,
}

pub(crate) fn run_table(client: &AptosClient, command: TableCommand) -> Result<()> {
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
            crate::print_pretty_json(&value)
        }
    }
}

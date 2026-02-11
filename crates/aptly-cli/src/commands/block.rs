use anyhow::{anyhow, Result};
use aptly_aptos::AptosClient;
use clap::{Args, Subcommand};

#[derive(Args)]
pub(crate) struct BlockCommand {
    #[command(subcommand)]
    pub(crate) command: Option<BlockSubcommand>,
    /// Block height used when no subcommand is provided.
    pub(crate) height: Option<String>,
    /// Include full transaction payloads in block response.
    #[arg(long, default_value_t = false)]
    pub(crate) with_transactions: bool,
}

#[derive(Subcommand)]
pub(crate) enum BlockSubcommand {
    #[command(name = "by-version", about = "Fetch block by ledger version")]
    ByVersion(ByVersionArgs),
}

#[derive(Args)]
pub(crate) struct ByVersionArgs {
    /// Ledger version to resolve containing block.
    pub(crate) version: String,
    /// Include full transaction payloads in block response.
    #[arg(long, default_value_t = false)]
    pub(crate) with_transactions: bool,
}

pub(crate) fn run_block(client: &AptosClient, command: BlockCommand) -> Result<()> {
    match command.command {
        Some(BlockSubcommand::ByVersion(args)) => {
            let path = format!(
                "/blocks/by_version/{}?with_transactions={}",
                args.version, args.with_transactions
            );
            let value = client.get_json(&path)?;
            crate::print_pretty_json(&value)
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
            crate::print_pretty_json(&value)
        }
    }
}

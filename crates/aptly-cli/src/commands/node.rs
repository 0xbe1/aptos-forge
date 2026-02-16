use anyhow::Result;
use aptly_aptos::AptosClient;
use clap::{Args, Subcommand};

#[derive(Args)]
#[command(
    after_help = "Examples:\n  aptly node ledger\n  aptly node health\n  aptly --rpc-url https://rpc.sentio.xyz/aptos/v1 node estimate-gas-price"
)]
pub(crate) struct NodeCommand {
    #[command(subcommand)]
    pub(crate) command: NodeSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum NodeSubcommand {
    #[command(about = "Get ledger info from `/`")]
    Ledger,
    #[command(about = "Get OpenAPI spec JSON")]
    Spec,
    #[command(about = "Check node health")]
    Health,
    #[command(about = "Get node build/runtime info")]
    Info,
    #[command(name = "estimate-gas-price", about = "Estimate current gas price")]
    EstimateGasPrice,
}

pub(crate) fn run_node(client: &AptosClient, command: NodeCommand) -> Result<()> {
    let value = match command.command {
        NodeSubcommand::Ledger => client.get_json("/")?,
        NodeSubcommand::Spec => client.get_json("/spec.json")?,
        NodeSubcommand::Health => client.get_json("/-/healthy")?,
        NodeSubcommand::Info => client.get_json("/info")?,
        NodeSubcommand::EstimateGasPrice => client.get_json("/estimate_gas_price")?,
    };

    crate::print_pretty_json(&value)
}

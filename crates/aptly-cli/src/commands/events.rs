use anyhow::Result;
use aptly_aptos::AptosClient;
use clap::Args;

#[derive(Args)]
#[command(
    after_help = "Examples:\n  aptly events 0x1 0 --limit 10\n  aptly events 0x1 0 --start 100 --limit 25"
)]
pub(crate) struct EventsCommand {
    /// Account address that owns the event handle.
    #[arg(value_name = "ADDRESS")]
    pub(crate) address: String,
    /// Event handle creation number.
    #[arg(value_name = "CREATION_NUMBER")]
    pub(crate) creation_number: String,
    /// Maximum number of events to return.
    #[arg(long, default_value_t = 25)]
    pub(crate) limit: u64,
    /// Start cursor (ledger version offset).
    #[arg(long, default_value_t = 0)]
    pub(crate) start: u64,
}

pub(crate) fn run_events(client: &AptosClient, command: EventsCommand) -> Result<()> {
    let mut path = format!(
        "/accounts/{}/events/{}?limit={}",
        command.address, command.creation_number, command.limit
    );
    if command.start > 0 {
        path.push_str(&format!("&start={}", command.start));
    }

    let value = client.get_json(&path)?;
    crate::print_pretty_json(&value)
}

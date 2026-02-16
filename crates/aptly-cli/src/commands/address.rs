use anyhow::{anyhow, Context, Result};
use clap::Args;
use reqwest::StatusCode;
use std::collections::HashMap;

const LABELS_URL: &str =
    "https://raw.githubusercontent.com/ThalaLabs/aptos-labels/main/mainnet.json";

#[derive(Args)]
#[command(after_help = "Examples:\n  aptly address thala\n  aptly address panora")]
pub(crate) struct AddressCommand {
    /// Case-insensitive substring to match against known labels.
    #[arg(value_name = "QUERY")]
    pub(crate) query: String,
}

pub(crate) fn run_address(command: AddressCommand) -> Result<()> {
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

    crate::print_serialized(&matches)
}

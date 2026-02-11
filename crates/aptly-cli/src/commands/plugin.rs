use crate::plugin_tools::{
    discover_aptos_script_compose, discover_aptos_tracer, discover_move_decompiler,
    doctor_aptos_script_compose, doctor_aptos_tracer, doctor_move_decompiler,
};
use anyhow::{anyhow, Result};
use clap::{Args, Subcommand};

#[derive(Args)]
pub(crate) struct PluginCommand {
    #[command(subcommand)]
    pub(crate) command: PluginSubcommand,
}

#[derive(Subcommand)]
pub(crate) enum PluginSubcommand {
    #[command(about = "List discovered plugin binaries and metadata")]
    List,
    #[command(about = "Run health checks for plugin binaries")]
    Doctor(PluginDoctorArgs),
}

#[derive(Args)]
pub(crate) struct PluginDoctorArgs {
    /// Explicit move-decompiler binary path.
    #[arg(long = "decompiler-bin")]
    pub(crate) decompiler_bin: Option<String>,
    /// Explicit aptos-tracer binary path.
    #[arg(long = "tracer-bin")]
    pub(crate) tracer_bin: Option<String>,
    /// Explicit aptos-script-compose binary path.
    #[arg(long = "script-compose-bin")]
    pub(crate) script_compose_bin: Option<String>,
}

pub(crate) fn run_plugin(command: PluginCommand) -> Result<()> {
    match command.command {
        PluginSubcommand::List => {
            let plugins = vec![
                discover_move_decompiler(None),
                discover_aptos_tracer(None),
                discover_aptos_script_compose(None),
            ];
            crate::print_serialized(&plugins)
        }
        PluginSubcommand::Doctor(args) => {
            let reports = vec![
                doctor_move_decompiler(args.decompiler_bin.as_deref()),
                doctor_aptos_tracer(args.tracer_bin.as_deref()),
                doctor_aptos_script_compose(args.script_compose_bin.as_deref()),
            ];
            let ok = reports.iter().all(|report| report.all_ok());
            crate::print_serialized(&reports)?;
            if ok {
                Ok(())
            } else {
                Err(anyhow!(
                    "plugin doctor found issues; see install_hint for remediation"
                ))
            }
        }
    }
}

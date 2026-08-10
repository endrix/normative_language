use std::process::ExitCode;

use clap::Parser;
use ndf_core::version::{CLI_STABILITY, FORMAT_VERSION, IR_VERSION, PLUGIN_API, TOOL_VERSION};
use serde::Serialize;

use crate::args::Arguments;
use crate::envelope::Envelope;

mod args;
mod envelope;
mod run;

#[derive(Serialize)]
struct MachineVersion<'a> {
    cli_stability: &'a str,
    format_version: &'a str,
    ir_version: &'a str,
    plugin_api: &'a str,
    tool_version: &'a str,
}

fn main() -> ExitCode {
    let arguments = Arguments::parse();
    if arguments.machine_version {
        let payload = MachineVersion {
            cli_stability: CLI_STABILITY,
            format_version: FORMAT_VERSION,
            ir_version: IR_VERSION,
            plugin_api: PLUGIN_API,
            tool_version: TOOL_VERSION,
        };
        println!(
            "{}",
            serde_json::to_string(&payload).expect("machine version must serialize")
        );
        return ExitCode::SUCCESS;
    }
    let command = arguments.command_label();
    let envelope = match run::run(arguments) {
        Ok(envelope) => envelope,
        Err(error) => Envelope::failure(command, error.to_string()),
    };
    println!(
        "{}",
        serde_json::to_string(&envelope).expect("command envelope must serialize")
    );
    if envelope.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

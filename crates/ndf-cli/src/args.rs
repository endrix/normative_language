use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "ndf", arg_required_else_help = true)]
pub struct Arguments {
    #[arg(long)]
    pub machine_version: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

impl Arguments {
    pub fn command_label(&self) -> &'static str {
        match self.command.as_ref() {
            Some(Command::Build { .. }) => "build",
            Some(Command::Check { .. }) => "check",
            Some(Command::Show { .. }) => "show",
            Some(Command::Trace { .. }) => "trace",
            Some(Command::Report { command }) => match command {
                ReportCommand::Coverage { .. } => "report coverage",
                ReportCommand::Dependencies { .. } => "report dependencies",
            },
            Some(Command::Diff { .. }) => "diff",
            Some(Command::Impact { command }) => match command {
                ImpactCommand::PtoRelease { .. } => "impact pto-release",
            },
            Some(Command::Export { .. }) => "export",
            Some(Command::Id { .. }) => "id allocate",
            Some(Command::Fix { .. }) => "fix",
            None => "unknown",
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Build {
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    Check {
        #[arg(long)]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    Show {
        uri: String,
        #[arg(long, default_value = "graph.sqlite")]
        index: PathBuf,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    Trace {
        uri: String,
        #[arg(long = "edge")]
        edges: Vec<String>,
        #[arg(long, default_value = "graph.sqlite")]
        index: PathBuf,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    Report {
        #[command(subcommand)]
        command: ReportCommand,
    },
    Diff {
        before: PathBuf,
        after: PathBuf,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    Impact {
        #[command(subcommand)]
        command: ImpactCommand,
    },
    Export {
        #[arg(long)]
        index: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, value_enum)]
        format: ExportFormat,
    },
    Id {
        #[command(subcommand)]
        command: IdCommand,
    },
    Fix {
        paths: Vec<PathBuf>,
        #[arg(long)]
        generated_only: bool,
        #[arg(long)]
        check: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ImpactCommand {
    PtoRelease {
        #[arg(long)]
        before: PathBuf,
        #[arg(long)]
        after: PathBuf,
        #[arg(long)]
        consumer_index: PathBuf,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
}

#[derive(Debug, Subcommand)]
pub enum ReportCommand {
    Coverage {
        #[arg(long, default_value = "graph.sqlite")]
        index: PathBuf,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
    Dependencies {
        #[arg(long)]
        root: PathBuf,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
}

#[derive(Debug, Subcommand)]
pub enum IdCommand {
    Allocate {
        prefix: String,
        #[arg(long, default_value = "graph.sqlite")]
        index: PathBuf,
        #[arg(long, value_enum, default_value_t = JsonFormat::Json)]
        format: JsonFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum JsonFormat {
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExportFormat {
    #[value(name = "ndf-json")]
    NdfJson,
}

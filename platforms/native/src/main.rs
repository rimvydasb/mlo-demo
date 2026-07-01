use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "voxel-mlm", about = "Voxel Mega-Lo-Mania — dev inspector")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Inspect {
        #[arg(long, default_value = "42")]
        seed: u64,
    },
    Screenshot {
        #[arg(long, default_value = "42")]
        seed: u64,
        #[arg(long, default_value = "screenshot.png")]
        out: std::path::PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Inspect { seed }      => voxel_app::run_inspector(seed),
        Command::Screenshot { seed, out } => voxel_app::run_screenshot(seed, out),
    }
}

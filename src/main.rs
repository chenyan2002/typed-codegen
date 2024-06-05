use anyhow::Result;
use clap::Parser;
use env_logger::Env;
use std::path::PathBuf;

mod audit;
mod candid;
mod load_cargo;
mod utils;

#[derive(Parser)]
pub struct Options {
    #[arg(short, long, default_value = ".")]
    /// The path for Cargo project root.
    pub manifest_path: PathBuf,
    #[arg(short, long)]
    /// Package to process
    pub package: Option<String>,
    #[arg(long)]
    /// Do not activate the `default` feature.
    pub no_default_features: bool,
    #[arg(long)]
    /// Activate all features.
    pub all_features: bool,
    #[arg(long, conflicts_with("all_features"))]
    /// List of features to activate
    pub features: Vec<String>,
    #[arg(hide = true, long)]
    pub expand_proc_macros: bool,
}

#[derive(Parser)]
enum Command {
    /// Check if dependent crates has any unsafe functions or exposes any canister endpoints.
    Audit {
        #[command(flatten)]
        options: Options,
    },
    /// Export Candid interface from Rust project
    Candid {
        #[command(flatten)]
        options: Options,
    },
}

fn main() -> Result<()> {
    let env = Env::default().default_filter_or("debug");
    env_logger::Builder::from_env(env)
        .format_target(false)
        .init();
    match Command::parse() {
        Command::Audit { mut options } => {
            options.expand_proc_macros = true;
            let (db, vfs, target) = load_cargo::load_cargo_project(&options)?;
            let crates = load_cargo::find_non_root_crates(&db, &vfs, &target);
            for krate in crates {
                let mut builder = audit::Builder::new(&db, krate);
                builder.build();
            }
        }
        Command::Candid { mut options } => {
            options.expand_proc_macros = false;
            let (db, vfs, target) = load_cargo::load_cargo_project(&options)?;
            let krate = load_cargo::find_root_crate(&db, &vfs, &target)?;
            let mut builder = candid::Builder::new(&db, krate);
            builder.build();
            println!("{}", builder.emit_methods());
        }
    }
    Ok(())
}

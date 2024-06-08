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
        #[arg(short, long)]
        /// Trace unsafe functions from the main package. If false, scan external dependencies for import/export functions.
        trace_functions: bool,
        #[arg(short, long)]
        /// List of whitelisted crates.
        whitelist: Vec<String>,
    },
    /// Export Candid interface from Rust project
    Candid {
        #[command(flatten)]
        options: Options,
    },
}

fn main() -> Result<()> {
    use load_cargo::{
        find_crate, find_non_root_crates, find_whitelisted_crates, load_cargo_project,
    };
    let env = Env::default().default_filter_or("info");
    env_logger::Builder::from_env(env)
        .format_target(false)
        .init();
    match Command::parse() {
        Command::Audit {
            mut options,
            trace_functions,
            whitelist,
        } => {
            use audit::Mode;
            options.expand_proc_macros = true;
            let (ws, db, vfs, target) = load_cargo_project(&options)?;
            let whitelist = find_whitelisted_crates(&ws, &db, &vfs, &whitelist)?;
            if trace_functions {
                let krate = find_crate(&db, &vfs, &target)?;
                let mut builder = audit::Builder::new(&db, krate, whitelist, Mode::TraceFunctions);
                builder.build();
            } else {
                let crates = find_non_root_crates(&db, &vfs, &target);
                for krate in crates {
                    let mut builder =
                        audit::Builder::new(&db, krate, whitelist.clone(), Mode::ScanExports);
                    builder.build();
                }
            }
        }
        Command::Candid { mut options } => {
            options.expand_proc_macros = false;
            let (_, db, vfs, target) = load_cargo_project(&options)?;
            let krate = find_crate(&db, &vfs, &target)?;
            let mut builder = candid::Builder::new(&db, krate);
            builder.build();
            println!("{}", builder.emit_methods());
        }
    }
    Ok(())
}

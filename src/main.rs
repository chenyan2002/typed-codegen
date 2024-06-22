use anyhow::Result;
use clap::Parser;
use console::Style;
use env_logger::Env;
use indicatif::{HumanDuration, MultiProgress, ProgressBar, ProgressStyle};
use std::path::PathBuf;

mod audit;
mod bindgen;
mod candid;
mod check;
mod load_cargo;
mod utils;

#[derive(Parser)]
struct App {
    #[arg(hide=true, value_parser=clap::builder::PossibleValuesParser::new(["canister"]))]
    dummy: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Parser)]
struct Options {
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
    #[arg(short, long)]
    /// Disable progress bar, and use log
    pub verbose: bool,
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
        #[arg(short, long, num_args = 1.., value_delimiter = ',', default_value = "ic0,ic-cdk")]
        /// List of whitelisted crates.
        whitelist: Vec<String>,
    },
    /// Export Candid interface from Rust project
    Candid {
        #[command(flatten)]
        options: Options,
    },
    /// Generate Rust bindings from Candid interface
    Bindgen {
        #[arg(short, long, default_value = ".")]
        /// The path for canister.toml file
        canister_path: PathBuf,
    },
}
impl Command {
    fn is_verbose(&self) -> bool {
        match self {
            Command::Audit { options, .. } => options.verbose,
            Command::Candid { options, .. } => options.verbose,
            Command::Bindgen { .. } => true,
        }
    }
}

fn main() -> Result<()> {
    use load_cargo::{
        find_crate, find_non_root_crates, find_whitelisted_crates, load_cargo_project,
    };
    let cmd = App::parse().command;
    let is_verbose = cmd.is_verbose();
    let env = if is_verbose {
        Env::default().default_filter_or("info")
    } else {
        Env::default().default_filter_or("warn")
    };
    env_logger::Builder::from_env(env)
        .format_target(false)
        .init();
    let bars = MultiProgress::new();
    if is_verbose {
        bars.set_draw_target(indicatif::ProgressDrawTarget::hidden());
    }
    let start = std::time::Instant::now();
    match cmd {
        Command::Audit {
            mut options,
            trace_functions,
            whitelist,
        } => {
            use audit::Mode;
            options.expand_proc_macros = true;
            let (ws, db, vfs, target) = load_cargo_project(&options, &bars)?;
            let whitelist = find_whitelisted_crates(&ws, &db, &vfs, &whitelist)?;
            let mut size = 0;
            if trace_functions {
                let krate = find_crate(&db, &vfs, &target)?;
                let mut builder = audit::Builder::new(
                    &bars,
                    is_verbose,
                    &db,
                    krate,
                    whitelist,
                    Mode::TraceFunctions,
                );
                builder.build();
                size += builder.visited.len();
            } else {
                let crates = find_non_root_crates(&db, &vfs, &target);
                let bar = bars.add(ProgressBar::new(crates.len() as u64));
                bar.set_style(
                    ProgressStyle::with_template(
                        "{prefix:>12.cyan.bold} [{bar:57.green}] {pos}/{len}",
                    )
                    .unwrap()
                    .progress_chars("=> "),
                );
                bar.set_prefix("Scanning");
                for krate in crates {
                    bar.inc(1);
                    let mut builder = audit::Builder::new(
                        &bars,
                        is_verbose,
                        &db,
                        krate,
                        whitelist.clone(),
                        Mode::ScanExports,
                    );
                    builder.build();
                    size += builder.visited.len();
                }
                bar.finish_and_clear();
            }
            println!(
                "{:>12} auditing {} functions in {}",
                Style::new().green().bold().apply_to("Finished"),
                size,
                HumanDuration(start.elapsed())
            );
        }
        Command::Bindgen { canister_path } => bindgen::run(&canister_path)?,
        Command::Candid { mut options } => {
            options.expand_proc_macros = false;
            let (_, db, vfs, target) = load_cargo_project(&options, &bars)?;
            let krate = find_crate(&db, &vfs, &target)?;
            let mut builder = candid::Builder::new(&db, krate);
            builder.build();
            println!("{}", builder.emit_methods());
        }
    }
    bars.clear()?;
    Ok(())
}

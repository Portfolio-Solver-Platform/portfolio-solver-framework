mod ai;
mod args;
mod config;
mod fzn_to_features;
mod insert_objective;
mod logging;
mod model_parser;
mod msc_discovery;
mod mzn_to_fzn;
mod scheduler;
mod solver_manager;
mod solver_output;
mod static_schedule;
mod sunny;

use std::process::exit;

use crate::ai::SimpleAi;
use crate::args::{Ai, parse_ai_config};
use crate::config::Config;
use crate::sunny::sunny;
use args::Args;
use clap::Parser;
use tokio_util::sync::CancellationToken;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    logging::init(args.debug_verbosity);
    
    // Discover all .msc files and parse solver metadata when the program loads
    let solver_metadata = match msc_discovery::discover_solver_metadata(&args.minizinc_exe).await {
        Ok(metadata) => {
            logging::info!("Discovered solver metadata for {} solver(s)", metadata.len());
            metadata
        }
        Err(e) => {
            logging::warning!("Failed to discover solver metadata: {}", e);
            msc_discovery::SolverMetadataMap::new()
        }
    };
    
    let config = Config::default();
    let token = CancellationToken::new();
    let token_signal = token.clone();

    ctrlc::set_handler(move || {
        token_signal.cancel();
    })
    .expect("Error setting Ctrl-C handler");

    match args.ai {
        Ai::Simple => tokio::select! {
            _ = sunny(args, SimpleAi {}, config, solver_metadata, token.clone()) => {},
            _ = token.cancelled() => {}
        },
        Ai::CommandLine => {
            let ai_config = parse_ai_config(args.ai_config.as_deref());
            let Some(command) = ai_config.get("command") else {
                logging::error_msg!(
                    "'command' not provided in AI configuration when basic commandline AI has been specified"
                );
                exit(1);
            };

            let ai = crate::ai::commandline::Ai::new(command.clone(), args.debug_verbosity);
            tokio::select! {
                _ = sunny(args, ai, config, solver_metadata, token.clone()) => {},
                _ = token.cancelled() => {}
            }
        }
    }
}

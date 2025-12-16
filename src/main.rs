mod ai;
mod args;
mod config;
mod fzn_to_features;
mod model_parser;
mod mzn_to_fzn;
mod scheduler;
mod solver_manager;
mod solver_output;
mod sunny;

use std::process::exit;

use crate::ai::SimpleAi;
use crate::args::{Ai, parse_ai_config};
use crate::config::Config;
use crate::sunny::sunny;
use args::Args;
use clap::Parser;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    let config = Config::default();

    match args.ai {
        Ai::Simple => sunny(args, SimpleAi {}, config).await,
        Ai::BasicCommandLine => {
            let ai_config = parse_ai_config(args.ai_config.as_deref());
            let Some(command) = ai_config.get("command") else {
                eprintln!(
                    "'command' not provided in AI configuration when basic commandline AI has been specified"
                );
                exit(1);
            };

            let ai = crate::ai::commandline::Ai::new(command.clone(), args.debug_verbosity);
            sunny(args, ai, config).await;
        }
    }
}

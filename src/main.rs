mod input;
use clap::Parser;
use input::{Args, OutputMode};
mod minizinc_runner;
use minizinc_runner::run;
mod solver_output;

fn main() {
    let args = Args::parse();

    if let Some(n) = args.threads {
        run(&args, "gecode", n, 1000.0);
    } else {
        println!("Running with default threads");
    }

    match args.output_mode {
        Some(OutputMode::Dzn) => {
            println!("output in dzn")
        }
        None => {}
    }
}

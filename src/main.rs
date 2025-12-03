#![allow(warnings)]

mod ai;
mod input;
use clap::Parser;
use input::{Args, OutputMode};
mod minizinc_runner;
use crate::ai::ai;
use minizinc_runner::{cleanup_handler, run};
use std::sync::mpsc;
mod solver_output;

fn main() {
    let args = Args::parse();
    let (tx, rx) = mpsc::channel::<String>();

    let running_processes = cleanup_handler();

    if let Some(n) = args.threads {
        let schedule = ai(n);
        for elem in schedule {
            run(
                &args,
                &elem.solver,
                elem.cores,
                1000000000.0,
                tx.clone(),
                running_processes.clone(),
            )
            .expect("fail to run");
        }
        drop(tx); // drop original as only the clones are used
        for msg in rx {
            println!("{msg}");
        }
    } else {
        println!("Running with default threads");
    }
}

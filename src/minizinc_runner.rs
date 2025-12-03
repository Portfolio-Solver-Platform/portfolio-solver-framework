use crate::input::{Args, OutputMode};
use crate::solver_output::{Output, OutputKind};
use command_group::{CommandGroup, Signal};
use std::borrow::Borrow;
use std::io;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

pub fn run(args: &Args, solver: &str, num_cores: usize, time_limit: f32) -> io::Result<()> {
    let mut cmd = Command::new("minizinc");
    cmd.arg("--solver").arg(solver);
    cmd.arg(&args.model);

    if let Some(data_path) = &args.data {
        cmd.arg(data_path);
    }

    cmd.arg("-i");
    cmd.arg("--json-stream");
    cmd.arg("--output-mode").arg("json");
    cmd.arg("-f");
    cmd.arg("--output-objective");
    cmd.arg("--time-limit").arg(format!("{time_limit}"));

    if args.output_objective {
        cmd.arg("--output-objective");
    }

    if args.ignore_search {
        cmd.arg("-f");
    }
    cmd.arg(format!("-p {num_cores}"));

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .group()
        .spawn()?;
    let stdout = child
        .inner()
        .stdout
        .take()
        .expect("Failed to capture stdout");

    let (tx, rx) = mpsc::channel::<Output>();

    let reader_handler = thread::spawn(move || {
        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            match line {
                Ok(l) => {
                    let output = Output::parse(l.borrow());
                    tx.send(output).unwrap();
                }
                Err(e) => eprintln!("Error reading line: {}", e),
            }
        }
    });

    // let mut msg;
    for msg in rx {
        if msg.kind == OutputKind::Optimal {
            println!("OPTIMAL: {}", msg.original_output);
            return Ok(());
        } else {
            println!("NOT OPTIMAL: {}", msg.original_output);
        }
    }
    // println!("OPTIMAL: {}", msg.original_output);

    Ok(())
}

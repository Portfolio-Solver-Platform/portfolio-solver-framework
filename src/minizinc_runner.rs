use crate::input::{Args, OutputMode};
use crate::solver_output;
use command_group::{CommandGroup, Signal};
use std::borrow::Borrow;
use std::io;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
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

    let reader_handler = thread::spawn(move || {
        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            match line {
                Ok(l) => {
                    let parsed_output = solver_output::Output::parse(l.borrow());
                }
                Err(e) => eprintln!("Error reading line: {}", e),
            }
        }
    });

    // if output.status.success() {
    //     let stdout = String::from_utf8_lossy(&output.stdout);
    //     println!("Solver Output:\n{}", stdout);

    //     let parsed_output = solver_output::Output::parse(stdout.borrow());
    // } else {
    //     let stderr: std::borrow::Cow<'_, str> = String::from_utf8_lossy(&output.stderr);
    //     eprintln!("Error running solver:\n{}", stderr);
    // }

    Ok(())
}

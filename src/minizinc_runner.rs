use crate::input::{Args, OutputMode};
use crate::solver_output::{Output, Solution};
use command_group::{CommandGroup, GroupChild};
use kill_tree::blocking::kill_tree;
use std::io;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

pub fn cleanup_handler() -> Arc<Mutex<Vec<GroupChild>>> {
    let running_processes: Arc<Mutex<Vec<GroupChild>>> = Arc::new(Mutex::new(Vec::new()));
    let processes_for_signal = running_processes.clone();

    ctrlc::set_handler(move || {
        let pids = processes_for_signal.lock().unwrap();

        for child in pids.iter() {
            // kill the minizinc solver plus all the processes it spawned (including grandchildren)
            let process_id = child.id();
            let _ = kill_tree(process_id);
        }

        // Exit the program safely
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");
    return running_processes;
}

pub fn run(
    args: &Args,
    solver: &str,
    num_cores: usize,
    time_limit: f32,
    tx: Sender<String>,
    running_processes: Arc<Mutex<Vec<GroupChild>>>,
) -> io::Result<()> {
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
    cmd.arg("-p").arg(num_cores.to_string());

    let mut group_child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .group_spawn()?;

    let stdout = group_child
        .inner()
        .stdout
        .take()
        .expect("Failed to capture stdout");

    {
        let mut pids = running_processes.lock().unwrap();
        pids.push(group_child);
    }

    let _ = thread::spawn(move || {
        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            match line {
                Ok(l) => {
                    // let output = Output::parse(l.borrow()).expect("failed to parse line");
                    let output = l;
                    tx.send(output).expect("could not send message");
                }
                Err(e) => eprintln!("Error reading line: {}", e),
            }
        }
    });

    Ok(())
}

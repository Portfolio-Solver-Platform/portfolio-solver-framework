use crate::input::Args;
use futures::future::join_all;
use kill_tree;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::collections::{HashMap, HashSet};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
pub struct ScheduleElement {
    pub solver: String,
    pub id: usize,
    pub cores: usize,
}

impl ScheduleElement {
    pub fn new(solver: String, cores: usize, id: usize) -> Self {
        Self { solver, id, cores }
    }
}

pub type Schedule = Vec<ScheduleElement>;

#[derive(Debug)]
pub enum Error {
    KillTree(kill_tree::Error),
    InvalidSolver(String),
    Io(std::io::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<kill_tree::Error> for Error {
    fn from(value: kill_tree::Error) -> Self {
        Error::KillTree(value)
    }
}

#[derive(Debug)]
struct Msg {}

impl Msg {
    fn new() -> Self {
        Msg {}
    }
}

pub struct Scheduler {
    tx: mpsc::UnboundedSender<Msg>,
    running_pids: Arc<Mutex<HashSet<u32>>>,
    solver_to_pid: Arc<Mutex<HashMap<usize, u32>>>,
    args: Args,
}

impl Scheduler {
    pub fn new(args: Args) -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<Msg>();
        let running_pids = cleanup_handler();
        let pids_for_background = running_pids.clone();

        tokio::spawn(async move {
            let mut receiver = rx;

            while let Some(msg) = receiver.recv().await {
                println!("{:?}", msg);
            }

            let pids = pids_for_background.lock().await;
            for pid in pids.iter() {
                // kill_tree is async, await it
                let _ = kill_tree::tokio::kill_tree(*pid).await;
            }
            std::process::exit(0);
        });

        Self {
            tx,
            running_pids,
            solver_to_pid: Arc::new(Mutex::new(HashMap::new())),
            args,
        }
    }

    async fn start_solver(&mut self, elem: ScheduleElement) -> std::io::Result<()> {
        let mut cmd = Command::new("minizinc");
        cmd.arg("--solver").arg(&elem.solver);
        cmd.arg(&self.args.model);

        if let Some(data_path) = &self.args.data {
            cmd.arg(data_path);
        }

        cmd.arg("-i");
        cmd.arg("--json-stream");
        cmd.arg("--output-mode").arg("json");
        cmd.arg("-f");
        cmd.arg("--output-objective");

        if self.args.output_objective {
            cmd.arg("--output-objective");
        }

        if self.args.ignore_search {
            cmd.arg("-f");
        }
        cmd.arg("-p").arg(elem.cores.to_string());

        #[cfg(unix)]
        cmd.process_group(0); // let os give it a process id

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        let pid = child.id().expect("Child has no PID");
        {
            let mut map = self.solver_to_pid.lock().await;
            map.insert(elem.id, pid);
        }
        {
            let mut pids = self.running_pids.lock().await;
            pids.insert(pid);
        }

        let stdout = child.stdout.take().expect("Failed stdout");
        let stderr = child.stderr.take().expect("Failed stderr");

        let tx_clone = self.tx.clone();
        tokio::spawn(async move {
            Self::handle_solver_stdout(stdout, tx_clone).await;
        });

        tokio::spawn(async move { Self::handle_solver_stderr(stderr).await });

        let running_pids_clone = self.running_pids.clone(); // clones the reference to the HashSet, not the actual set
        let solver_to_pid_clone = self.solver_to_pid.clone();

        tokio::spawn(async move {
            let _ = child.wait().await;
            {
                let mut pids = running_pids_clone.lock().await;
                pids.remove(&pid);
            }
            {
                let mut map = solver_to_pid_clone.lock().await;
                map.remove(&elem.id);
            }
        });

        Ok(())
    }

    async fn handle_solver_stdout(
        stdout: tokio::process::ChildStdout,
        tx: tokio::sync::mpsc::UnboundedSender<Msg>,
    ) {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Ok(Some(_)) = lines.next_line().await {
            let msg = Msg::new();

            if let Err(e) = tx.send(msg) {
                eprintln!("Could not send message, receiver dropped: {}", e);
                break;
            }
        }
    }

    async fn handle_solver_stderr(stderr: tokio::process::ChildStderr) {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            // maybe problem we only handle if the error is okay
            eprintln!("Minizinc Solver: {}", line);
        }
    }

    pub async fn start_solvers(
        &mut self,
        schedule: Schedule,
    ) -> std::result::Result<(), Vec<std::io::Error>> {
        let mut errors = Vec::new();

        for elem in schedule {
            if let Err(e) = self.start_solver(elem).await {
                errors.push(e);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    pub async fn stop_solver(&mut self, id: usize) -> std::result::Result<(), Error> {
        let pid = {
            let mut map = self.solver_to_pid.lock().await;
            match map.remove(&id) {
                Some(id) => id,
                None => {
                    return Err(Error::InvalidSolver(format!("Solver {id} was not running")));
                }
            }
        };

        kill_tree::tokio::kill_tree(pid).await?;

        /*
        let pgid = Pid::from_raw(-(pid_num as i32));

        // This kills minizinc AND the solver
        if let Err(errno) = signal::kill(pgid, Signal::SIGKILL) {
             // Handle "Process not found" errors gracefully, imply it's already dead
             if errno != nix::errno::Errno::ESRCH {
                 return Err(Error::Io(std::io::Error::from_raw_os_error(errno as i32)));
             }
        } */
        {
            let mut pids = self.running_pids.lock().await;
            pids.remove(&pid);
        }
        {
            let mut map = self.solver_to_pid.lock().await;
            map.remove(&id);
        }
        Ok(())
    }

    /// Filters errors that are invalid process ids, since we can assume that those pid do not exist
    pub async fn stop_all_solvers(&self) -> std::result::Result<(), Vec<Error>> {
        let mut running_pids_guard = self.running_pids.lock().await;
        let pids_to_kill: Vec<u32> = running_pids_guard.iter().cloned().collect();

        let kill_futures = pids_to_kill
            .iter()
            .map(|id| kill_tree::tokio::kill_tree(*id));

        let results = join_all(kill_futures).await;

        let mut errors = Vec::new();
        let mut pids_to_remove = Vec::new();

        for (pid, result) in pids_to_kill.into_iter().zip(results) {
            match result {
                Ok(_) => {
                    pids_to_remove.push(pid);
                }
                Err(err) => {
                    if let kill_tree::Error::InvalidProcessId { .. } = err {
                        pids_to_remove.push(pid);
                    } else {
                        errors.push(Error::from(err));
                    }
                }
            }
        }

        running_pids_guard.retain(|pid| !pids_to_remove.contains(pid));

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub async fn pause_solver(&self, id: usize) -> std::result::Result<(), Error> {
        self.send_signal(id, Signal::SIGSTOP).await
    }

    pub async fn resume_solver(&self, id: usize) -> std::result::Result<(), Error> {
        self.send_signal(id, Signal::SIGCONT).await
    }

    async fn send_signal(&self, id: usize, signal: Signal) -> std::result::Result<(), Error> {
        let pid = {
            let map = self.solver_to_pid.lock().await;
            match map.get(&id) {
                Some(&p) => p,
                None => return Err(Error::InvalidSolver(format!("Solver {id} not running"))),
            }
        };

        let pgid = Pid::from_raw(-(pid as i32));
        match signal::kill(pgid, signal) {
            Ok(_) => Ok(()),
            Err(errno) => {
                // Nix wraps errno into a proper Rust error
                Err(Error::Io(std::io::Error::from_raw_os_error(errno as i32)))
            }
        }
    }

    pub async fn suspend_all_solvers(&self) -> std::result::Result<(), Vec<Error>> {
        let mut errors = Vec::new();

        // We need to clone the keys to avoid holding the lock while awaiting
        let ids: Vec<usize> = {
            let map = self.solver_to_pid.lock().await;
            map.keys().cloned().collect()
        };

        for id in ids {
            if let Err(e) = self.pause_solver(id).await {
                errors.push(e);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn cleanup_handler() -> Arc<Mutex<HashSet<u32>>> {
    let running_processes: Arc<Mutex<HashSet<u32>>> = Arc::new(Mutex::new(HashSet::new()));
    let processes_for_signal = running_processes.clone();

    ctrlc::set_handler(move || {
        let pids = processes_for_signal.blocking_lock();

        for pid in pids.iter() {
            // kill the minizinc solver plus all the processes it spawned (including grandchildren)
            let _ = kill_tree::blocking::kill_tree(*pid);
        }

        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");
    return running_processes;
}

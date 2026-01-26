use std::collections::hash_map::Entry;
use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;
use tokio::sync::watch;
use tokio::sync::watch::Receiver;
use tokio_util::sync::CancellationToken;

use super::Conversion;
use super::compilation;
use crate::args;
use crate::args::RunArgs;
use crate::logging;

pub struct CompilationManager {
    args: Arc<RunArgs>,
    compilations: Arc<RwLock<HashMap<String, Compilation>>>,
}

#[derive(Clone, Debug)]
enum Compilation {
    Done(Arc<compilation::Result<Conversion>>),
    Started(Arc<StartedCompilation>),
}

#[derive(Debug)]
struct StartedCompilation {
    cancellation_token: CancellationToken,
    receiver: Receiver<Option<Arc<compilation::Result<Conversion>>>>,
}

impl CompilationManager {
    pub async fn is_started(&self, solver_name: &str) -> bool {
        self.compilations.read().await.contains_key(solver_name)
    }

    pub async fn start(&self, solver_name: String) {
        self.start_all([solver_name].into_iter()).await
    }

    pub async fn start_all(&self, solver_names: impl Iterator<Item = String>) {
        let new_solvers: Vec<_> = {
            let compilations = self.compilations.read().await;
            solver_names
                .filter(|name| !compilations.contains_key(name))
                .collect()
        };

        if self.args.verbosity >= args::Verbosity::Info {
            new_solvers.iter().for_each(|solver_name| logging::info!("Attempted to start compiling for '{solver_name}' even though it has already started compilation or is done compiling"));
        }

        let new_compilations = new_solvers.into_iter().map(|solver_name| {
            let cancellation_token = CancellationToken::new();
            let args = self.args.clone();
            let cancellation_token_clone = cancellation_token.clone();
            let name_clone = solver_name.clone();

            let compilations = self.compilations.clone();

            let (tx, rx) = watch::channel(None);

            tokio::spawn(async move {
                let compilation =
                    compilation::convert_mzn(&args, &solver_name, cancellation_token_clone).await;

                let compilation = Arc::new(compilation);

                {
                    compilations
                        .write()
                        .await
                        .insert(solver_name, Compilation::Done(compilation.clone()));
                }

                let _ = tx.send(Some(compilation));
            });

            (
                name_clone,
                StartedCompilation {
                    cancellation_token,
                    receiver: rx,
                },
            )
        });

        let mut compilations = self.compilations.write().await;
        for (name, compilation) in new_compilations {
            compilations.insert(name, Compilation::Started(Arc::new(compilation)));
        }
    }

    pub async fn get(
        &self,
        solver_name: &str,
        cancellation_token: Option<CancellationToken>,
    ) -> Option<Arc<compilation::Result<Conversion>>> {
        let compilation = {
            let compilations = self.compilations.read().await;
            compilations.get(solver_name).cloned()
        };

        let Some(compilation) = compilation else {
            logging::info!(
                "Attempted to get the compilation of solver '{solver_name}' when it has not been started"
            );
            return None;
        };

        match compilation {
            Compilation::Done(result) => Some(result),
            Compilation::Started(compilation) => {
                let mut rx = compilation.receiver.clone();
                let result = rx.wait_for(|value| value.is_some()).await;

                let Ok(value) = result else {
                    logging::error_msg!(
                        "sender closed the channel for '{solver_name}' before finishing compilation"
                    );
                    return None;
                };

                let Some(compilation) = value.clone() else {
                    logging::error_msg!("value is None despite waiting for it to be Some");
                    return None;
                };

                Some(compilation)
            }
        }
    }

    pub async fn stop_all(&self, solver_names: impl Iterator<Item = String>) {
        let mut compilations = self.compilations.write().await;

        for solver_name in solver_names {
            if let Entry::Occupied(compilation) = compilations.entry(solver_name) {
                match compilation.get() {
                    Compilation::Started(started_compilation) => {
                        started_compilation.cancellation_token.cancel();
                        let (solver_name, _) = compilation.remove_entry();
                        logging::info!("stopped the compilation for solver '{solver_name}'");
                    }
                    Compilation::Done(_) => {
                        logging::info!("attempted to stop a finished compilation for a solver");
                    }
                }
            } else {
                logging::error_msg!(
                    "attempted to stop the compilation for a solver but a compilation is not registered for that solver (neither as started or finished)"
                );
            }
        }
    }
}

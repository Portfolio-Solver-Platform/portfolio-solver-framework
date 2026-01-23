use std::{collections::HashMap, sync::Arc};

use futures::FutureExt;
use futures::future;
use tokio::sync::watch;
use tokio::sync::watch::Receiver;
use tokio::task::JoinError;
use tokio::{sync::RwLock, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use super::Conversion;
use super::compilation;
use crate::args;
use crate::args::RunArgs;
use crate::logging;
use crate::mzn_to_fzn::ConversionError;

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

            let is_done_token = CancellationToken::new();
            let is_done_token_clone = is_done_token.clone();

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
                    logging::error_msg!("sender closed the channel before finishing compilation");
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

    pub async fn stop_all(&self) {
        todo!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("compilation was cancelled")]
    Cancelled,
    #[error(transparent)]
    Conversion(#[from] ConversionError),
}

pub enum Cancellable<T> {
    Done(T),
    Cancelled,
}

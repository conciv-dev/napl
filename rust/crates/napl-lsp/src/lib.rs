//! `napl-lsp`: the NAPL language server, driven by the `napl lsp` subcommand over
//! stdio. It is a thin adapter over `napl-core`: hover, definition, references,
//! CodeLens, and diagnostics are all derived from the same map / attribution /
//! machine-layer state the CLI reads, so the two surfaces never diverge.

mod backend;
mod classify;
mod context;
mod convert;
mod diagnostics;
mod hover;
mod ml;
mod navigation;
mod state;

#[cfg(test)]
mod integration;
#[cfg(test)]
mod testkit;

use tower_lsp_server::{LspService, Server};

pub use backend::Backend;

/// The server version reported in `initialize`.
pub const VERSION: &str = "0.1.0";

/// Run the language server over stdio, blocking until the client disconnects.
pub fn run() -> std::io::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(serve());
    Ok(())
}

async fn serve() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}

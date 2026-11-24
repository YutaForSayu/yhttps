//! YhHTTPServer (`yhtps`) — a zero-config static & framework HTTP server.

mod auth;
mod banner;
mod body;
mod cli;
mod compress;
mod config;
mod fsutil;
mod listing;
mod logging;
mod proxy;
mod responder;
mod server;
mod state;
mod style;
#[cfg(feature = "tls")]
mod tls;

#[cfg(test)]
mod tests;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    let config = match config::Config::from_cli(cli) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("yhtps: error: {e:#}");
            return ExitCode::FAILURE;
        }
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("yhtps")
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("yhtps: error: cannot start async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(server::run(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("yhtps: error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

//! The accept loop, hyper service wiring and graceful shutdown.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, Result};
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::task::TaskTracker;

use crate::banner;
use crate::config::Config;
use crate::responder;
use crate::state::State;

pub async fn run(cfg: Config) -> Result<()> {
    let state = State::new(cfg)?;

    let addr = format!("{}:{}", state.cfg.host, state.cfg.port);
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("cannot bind to {addr} — is the port already in use?"))?;
    let local_addr = listener.local_addr()?;

    banner::print_startup(&state, local_addr)?;
    if state.cfg.open_browser {
        let url = banner::local_url(&state, local_addr);
        banner::open_in_browser(&url);
    }

    let tracker = TaskTracker::new();
    let mut shutdown = Box::pin(shutdown_signal());

    loop {
        tokio::select! {
            _ = &mut shutdown => {
                banner::print_shutdown(&state);
                break;
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, peer)) => {
                        let state = state.clone();
                        tracker.spawn(async move {
                            if let Err(e) = serve_connection(state, stream, peer).await {
                                tracingless_debug(&e);
                            }
                        });
                    }
                    Err(e) => {
                        // Connection-level errors (EMFILE, ECONNABORTED…) must
                        // never take the whole server down.
                        if state.log.color_enabled() {
                            eprintln!("\x1b[31maccept error:\x1b[0m {e}");
                        } else {
                            eprintln!("accept error: {e}");
                        }
                    }
                }
            }
        }
    }

    tracker.close();
    let _ = tokio::time::timeout(Duration::from_secs(5), tracker.wait()).await;
    Ok(())
}

fn tracingless_debug(e: &anyhow::Error) {
    if std::env::var_os("YHTPS_DEBUG").is_some() {
        eprintln!("connection error: {e:#}");
    }
}

async fn serve_connection(
    state: std::sync::Arc<State>,
    stream: TcpStream,
    peer: SocketAddr,
) -> Result<()> {
    let _ = stream.set_nodelay(true);

    #[cfg(feature = "tls")]
    if let Some(acceptor) = state.tls.clone() {
        let tls_stream = acceptor
            .accept(stream)
            .await
            .context("TLS handshake failed")?;
        return serve_hyper(state, TokioIo::new(tls_stream), peer).await;
    }

    serve_hyper(state, TokioIo::new(stream), peer).await
}

async fn serve_hyper<I>(
    state: std::sync::Arc<State>,
    io: TokioIo<I>,
    peer: SocketAddr,
) -> Result<()>
where
    I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let timeout = state.cfg.timeout;
    let service = service_fn(move |req| {
        let state = state.clone();
        async move { Ok::<_, Infallible>(responder::handle(state, req, peer).await) }
    });

    let mut builder = auto::Builder::new(TokioExecutor::new());
    if let Some(secs) = timeout {
        builder
            .http1()
            .header_read_timeout(Duration::from_secs(secs.max(1)));
    }
    builder
        .serve_connection(io, service)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

/// Resolves on SIGINT (all platforms) or SIGTERM (Unix).
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
                return;
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

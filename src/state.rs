//! Shared server state.

use std::sync::Arc;

use anyhow::Result;
use hyper::header::HeaderMap;

use crate::config::Config;
use crate::logging::{self, Logger};

#[cfg(feature = "tls")]
pub type Acceptor = tokio_rustls::TlsAcceptor;

/// HTTP client used for `--proxy`, lazily created.
pub type ProxyClient = hyper_util::client::legacy::Client<
    hyper_util::client::legacy::connect::HttpConnector,
    hyper::body::Incoming,
>;

pub struct State {
    pub cfg: Config,
    pub log: Logger,
    #[cfg(feature = "tls")]
    pub tls: Option<Acceptor>,
    /// Shown in the startup banner (e.g. the self-signed certificate note).
    pub tls_note: Option<String>,
    pub proxy: Option<ProxyClient>,
}

impl State {
    pub fn new(cfg: Config) -> Result<Arc<State>> {
        let log = Logger::new(cfg.log.clone());

        #[cfg(feature = "tls")]
        let (tls, tls_note) = match &cfg.ssl {
            Some(opts) => {
                let (acceptor, note) = crate::tls::build_acceptor(opts)?;
                (Some(acceptor), note)
            }
            None => (None, None),
        };
        #[cfg(not(feature = "tls"))]
        let tls_note: Option<String> = {
            if cfg.ssl.is_some() {
                anyhow::bail!(
                    "this build was compiled without TLS support — rebuild with features `tls` \
                     (i.e. without `--no-default-features`) to use --ssl/--cert/--key"
                );
            }
            None
        };

        let proxy = if cfg.proxy.is_some() {
            let mut connector = hyper_util::client::legacy::connect::HttpConnector::new();
            connector.enforce_http(false);
            let client =
                hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                    .build(connector);
            Some(client)
        } else {
            None
        };

        Ok(Arc::new(State {
            cfg,
            log,
            #[cfg(feature = "tls")]
            tls,
            tls_note,
            proxy,
        }))
    }

    /// Effective `Access-Control-Allow-Origin` value, if CORS is on.
    pub fn cors_origin(&self) -> Option<String> {
        self.cfg.cors.clone()
    }

    /// Client IP, honouring `--trust-proxy`.
    pub fn client_ip(&self, headers: &HeaderMap, peer: &str) -> String {
        logging::client_ip(headers, peer, self.cfg.log.trust_proxy)
    }

    /// Scheme used in the startup banner.
    pub fn scheme(&self) -> &'static str {
        if self.cfg.ssl.is_some() {
            "https"
        } else {
            "http"
        }
    }
}

//! TLS setup: load a PEM pair or mint a throwaway self-signed certificate.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;

use crate::config::SslOptions;

/// Build the TLS acceptor, plus an optional human-readable note.
pub fn build_acceptor(opts: &SslOptions) -> Result<(TlsAcceptor, Option<String>)> {
    let (certs, key, note) = match (&opts.cert, &opts.key) {
        (Some(cert), Some(key)) => (
            load_certs(cert)?,
            load_key(key)?,
            Some(format!("certificate from {}", cert.display())),
        ),
        _ => {
            let (cert_pem, key_pem) = generate_self_signed()?;
            (
                parse_certs(cert_pem.as_bytes())?,
                parse_key(key_pem.as_bytes())?,
                Some(
                    "self-signed certificate generated — the browser will warn until you accept it"
                        .to_string(),
                ),
            )
        }
    };

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("the certificate and key do not form a valid pair")?;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok((TlsAcceptor::from(std::sync::Arc::new(config)), note))
}

fn load_certs(path: &PathBuf) -> Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path)
        .with_context(|| format!("cannot open certificate '{}'", path.display()))?;
    let mut reader = BufReader::new(file);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("cannot parse certificate '{}'", path.display()))?;
    if certs.is_empty() {
        return Err(anyhow!("no certificates found in '{}'", path.display()));
    }
    Ok(certs)
}

fn load_key(path: &PathBuf) -> Result<PrivateKeyDer<'static>> {
    let file = File::open(path)
        .with_context(|| format!("cannot open private key '{}'", path.display()))?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)
        .with_context(|| format!("cannot parse private key '{}'", path.display()))?
        .ok_or_else(|| anyhow!("no private key found in '{}'", path.display()))
}

fn parse_certs(pem: &[u8]) -> Result<Vec<CertificateDer<'static>>> {
    let mut reader = BufReader::new(pem);
    let certs: Vec<CertificateDer<'static>> =
        rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
    if certs.is_empty() {
        return Err(anyhow!("generated certificate could not be parsed"));
    }
    Ok(certs)
}

fn parse_key(pem: &[u8]) -> Result<PrivateKeyDer<'static>> {
    let mut reader = BufReader::new(pem);
    rustls_pemfile::private_key(&mut reader)?
        .ok_or_else(|| anyhow!("generated private key could not be parsed"))
}

/// Mint a self-signed certificate covering `localhost` and every local IP.
fn generate_self_signed() -> Result<(String, String)> {
    use rcgen::{CertificateParams, DnType, KeyPair};

    let mut names: Vec<String> = vec!["localhost".to_string(), "127.0.0.1".to_string()];
    if let Ok(ifaces) = local_ip_address::list_afinet_netifas() {
        for (_n, ip) in ifaces {
            if !ip.is_loopback() {
                names.push(ip.to_string());
            }
        }
    }
    names.sort();
    names.dedup();

    let mut params = CertificateParams::new(names)
        .map_err(|e| anyhow!("cannot build certificate parameters: {e}"))?;
    params
        .distinguished_name
        .push(DnType::CommonName, "YhHTTPServer (self-signed)");
    params
        .distinguished_name
        .push(DnType::OrganizationName, "YhHTTPServer");

    let key_pair =
        KeyPair::generate().map_err(|e| anyhow!("cannot generate a private key: {e}"))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| anyhow!("cannot generate a self-signed certificate: {e}"))?;

    Ok((cert.pem(), key_pair.serialize_pem()))
}

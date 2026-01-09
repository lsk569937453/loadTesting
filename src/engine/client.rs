use http_body_util::Full;
use hyper::body::Bytes;
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use hyper_util::rt::TokioExecutor;
use rustls::crypto::ring::{DEFAULT_CIPHER_SUITES, default_provider};
use rustls::{ClientConfig, RootCertStore, crypto::CryptoProvider};
use webpki_roots::TLS_SERVER_ROOTS;

/// Create HTTP client with TLS support
pub fn create_http_client()
-> Result<Client<HttpsConnector<HttpConnector>, Full<Bytes>>, anyhow::Error> {
    let mut root_store = RootCertStore::empty();
    root_store.extend(TLS_SERVER_ROOTS.iter().cloned());
    let versions = rustls::DEFAULT_VERSIONS.to_vec();
    let tls_config = ClientConfig::builder_with_provider(
        CryptoProvider {
            cipher_suites: DEFAULT_CIPHER_SUITES.to_vec(),
            ..default_provider()
        }
        .into(),
    )
    .with_protocol_versions(&versions)?
    .with_root_certificates(root_store)
    .with_no_client_auth();

    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(tls_config)
        .https_or_http()
        .enable_http1()
        .enable_http2()
        .build();

    Ok(Client::builder(TokioExecutor::new()).build(https))
}

#![deny(clippy::pedantic)]

use argh::FromArgs;
use async_zip::tokio::read::fs::ZipFileReader;
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use tokio::{net::TcpListener, time::timeout};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        self,
        pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
    },
};

mod server;
#[cfg(test)]
mod tests;

/// a zipapp gemini server
#[derive(Debug, FromArgs)]
#[argh(help_triggers("--help"))]
struct Opt {
    /// address to listen on
    #[argh(option, default = "\"[::]:1965\".parse().unwrap()")]
    bind: SocketAddr,
    /// zip file to serve files from.
    ///
    /// defaults to the current binary in procfs, serving files from a
    /// zip file concatenated with itself
    #[argh(option, default = "\"/proc/self/exe\".parse().unwrap()")]
    zip: PathBuf,
    /// path to your tls certificate
    #[argh(positional)]
    cert: PathBuf,
    /// path to your tls private key.
    ///
    /// defaults to looking in the same file as your certificate,
    /// allowing both to be in one file
    #[argh(positional)]
    key: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let opt: Opt = argh::from_env();
    let zip = ZipFileReader::new(&opt.zip)
        .await
        .expect("failed to open zip");
    let srv = Arc::new(server::Server::from_zip(zip));
    let cert = CertificateDer::pem_file_iter(&opt.cert)
        .expect("could not open certificate")
        .collect::<Result<Vec<_>, _>>()
        .expect("could not parse certificate");
    let key = PrivateKeyDer::from_pem_file(opt.key.unwrap_or(opt.cert))
        .expect("could not open private key");
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert, key)
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind(&opt.bind).await.unwrap();

    println!("listening on {}", listener.local_addr().unwrap());

    loop {
        let (sock, _addr) = listener.accept().await.unwrap();
        let acceptor = acceptor.clone();
        let srv = srv.clone();

        tokio::spawn(async move {
            let Ok(Ok(stream)) = timeout(Duration::from_secs(10), acceptor.accept(sock)).await
            else {
                return;
            };

            srv.handle_connection(stream).await;
        });
    }
}

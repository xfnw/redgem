#![deny(clippy::pedantic)]

use async_zip::tokio::read::fs::ZipFileReader;
use clap::Parser;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{io::AsyncWriteExt, net::TcpListener};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        self,
        pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
    },
};

mod server;

#[derive(Debug, Parser)]
struct Opt {
    /// address to listen on
    #[arg(long, default_value = "[::]:1965")]
    bind: SocketAddr,
    /// zip file to serve files from
    ///
    /// defaults to the current binary, serving files from a zip file
    /// concatenated with itself
    #[arg(long, default_value = "/proc/self/exe")]
    zip: PathBuf,
    /// path to your tls certificate
    cert: PathBuf,
    /// path to your tls private key
    ///
    /// defaults to looking in the same file as your certificate,
    /// allowing both to be in one file
    key: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let opt = Opt::parse();
    let zip = ZipFileReader::new(&opt.zip).await.unwrap();
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
            let Ok(mut stream) = acceptor.accept(sock).await else {
                return;
            };

            srv.handle_connection(&mut stream).await;

            _ = stream.shutdown().await;
        });
    }
}

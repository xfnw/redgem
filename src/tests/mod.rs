use async_zip::tokio::read::fs::ZipFileReader;
use std::{
    net::{Ipv6Addr, SocketAddr},
    pin::Pin,
    sync::Arc,
};
use tokio::{
    io::{AsyncWriteExt, copy, sink},
    net::{TcpListener, TcpStream},
};
use tokio_rustls::{
    TlsAcceptor, TlsConnector,
    rustls::{
        ClientConfig, RootCertStore, ServerConfig,
        pki_types::{CertificateDer, PrivateKeyDer, ServerName, pem::PemObject},
    },
    server::TlsStream,
};

use crate::server::Server;

const CERT_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/tests/test.pem");
const KEY_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/tests/test.key");
const ZIP_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/tests/test.zip");

async fn serve_tls<F>(callback: F) -> SocketAddr
where
    F: Fn(TlsStream<TcpStream>) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>
        + Send
        + Clone
        + 'static,
{
    let cert = CertificateDer::pem_file_iter(CERT_PATH)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let key = PrivateKeyDer::from_pem_file(KEY_PATH).unwrap();
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert, key)
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("[::1]:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let (sock, _) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            let callback = callback.clone();

            tokio::spawn(async move {
                let stream = acceptor.accept(sock).await.unwrap();
                callback(stream).await;
            });
        }
    });

    addr
}

async fn get_notify(addr: SocketAddr, req: &[u8]) -> bool {
    let mut trust = RootCertStore::empty();
    trust
        .add(CertificateDer::from_pem_file(CERT_PATH).unwrap())
        .unwrap();
    let config = ClientConfig::builder()
        .with_root_certificates(trust)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(config));
    let sn = ServerName::from(Ipv6Addr::from_bits(1));
    let sock = TcpStream::connect(&addr).await.unwrap();
    let mut stream = connector.connect(sn, sock).await.unwrap();

    stream.write_all(req).await.unwrap();

    let mut sink = sink();
    copy(&mut stream, &mut sink).await.is_ok()
}

#[tokio::test]
async fn close_notify() {
    let zip = ZipFileReader::new(ZIP_PATH).await.unwrap();
    let srv = Arc::new(Server::from_zip(zip));
    let addr = serve_tls(move |s| {
        let srv = srv.clone();
        Box::pin(async move {
            srv.handle_connection(s).await;
        })
    })
    .await;
    assert!(get_notify(addr, b"gemini://localhost/\r\n").await);
    assert!(get_notify(addr, b"gemini://localhost/doesnotexist\r\n").await);
}

/// make sure rustls' behavior of not sending close_notify when [`TlsStream`] is dropped without
/// calling shutdown does not change. we need to not send it if we timeout before the client
/// consumes the whole response, to signify that the response has been truncated
#[tokio::test]
async fn no_shutdown() {
    let addr = serve_tls(|_| Box::pin(async {})).await;
    assert!(!get_notify(addr, b"gemini://localhost/\r\n").await);
}

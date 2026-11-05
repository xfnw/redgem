#![allow(clippy::unwrap_used)]

use async_zip::base::read1::seek::ZipArchiveReader;
use std::{
    net::{Ipv6Addr, SocketAddr},
    path::PathBuf,
    pin::Pin,
    sync::Arc,
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt, BufReader, copy},
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
use tokio_util::compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};

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

async fn request(addr: SocketAddr, req: &[u8]) -> Result<Vec<u8>, std::io::Error> {
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

    let mut out = Vec::new();
    copy(&mut stream, &mut out).await?;
    Ok(out)
}

#[tokio::test]
async fn index() {
    let zip = ZipArchiveReader::open(BufReader::new(File::open(ZIP_PATH).await.unwrap()).compat())
        .await
        .unwrap();
    let srv = Arc::new(Server::from_zip(
        zip.inner().clone(),
        PathBuf::from(ZIP_PATH),
    ));
    let addr = serve_tls(move |s| {
        let srv = srv.clone();
        Box::pin(async move {
            srv.handle_connection(s).await;
        })
    })
    .await;
    assert_eq!(
        request(addr, b"gemini://localhost/\r\n").await.unwrap(),
        b"20 text/gemini\r\nhewwo world\n"
    );
    assert_eq!(
        request(addr, b"gemini://localhost\r\n").await.unwrap(),
        b"20 text/gemini\r\nhewwo world\n"
    );
}

#[tokio::test]
async fn directory() {
    let zip = ZipArchiveReader::open(BufReader::new(File::open(ZIP_PATH).await.unwrap()).compat())
        .await
        .unwrap();
    let srv = Arc::new(Server::from_zip(
        zip.inner().clone(),
        PathBuf::from(ZIP_PATH),
    ));
    let addr = serve_tls(move |s| {
        let srv = srv.clone();
        Box::pin(async move {
            srv.handle_connection(s).await;
        })
    })
    .await;
    assert_eq!(
        request(addr, b"gemini://localhost/directory\r\n")
            .await
            .unwrap(),
        b"31 gemini://localhost/directory/\r\n"
    );
    assert_eq!(
        request(addr, b"gemini://localhost/directory/\r\n")
            .await
            .unwrap(),
        b"20 text/gemini\r\nmeow\n"
    );
}

#[tokio::test]
async fn length() {
    let zip = ZipArchiveReader::open(BufReader::new(File::open(ZIP_PATH).await.unwrap()).compat())
        .await
        .unwrap();
    let srv = Arc::new(Server::from_zip(
        zip.inner().clone(),
        PathBuf::from(ZIP_PATH),
    ));
    let addr = serve_tls(move |s| {
        let srv = srv.clone();
        Box::pin(async move {
            srv.handle_connection(s).await;
        })
    })
    .await;
    let mut hhhh = b"gemini://localhost/".to_vec();
    hhhh.extend_from_slice(&[b'h'; 1024]);
    let eol = b"\r\n";

    let mut short = hhhh[..1024].to_vec();
    short.extend_from_slice(eol);
    assert_eq!(
        request(addr, short.as_slice()).await.unwrap(),
        b"51 not found\r\n"
    );

    let mut long = hhhh[..1025].to_vec();
    long.extend_from_slice(eol);
    assert_eq!(
        request(addr, long.as_slice()).await.unwrap(),
        b"59 request too long\r\n"
    );
}

/// make sure rustls' behavior of not sending `close_notify` when [`TlsStream`] is dropped without
/// calling shutdown does not change. we need to not send it if we timeout before the client
/// consumes the whole response, to signify that the response has been truncated
#[tokio::test]
async fn no_shutdown() {
    let addr = serve_tls(|_| Box::pin(async {})).await;
    assert!(request(addr, b"gemini://localhost/\r\n").await.is_err());
}

/// make sure [`async_zip`] is fine with the runtime being switched out
#[test]
fn zip_swap_runtime() {
    let zip = {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let zip = runtime.block_on(async {
            ZipArchiveReader::open(BufReader::new(File::open(ZIP_PATH).await.unwrap()).compat())
                .await
                .unwrap()
        });
        assert_eq!(runtime.metrics().num_alive_tasks(), 0);
        zip
    };

    let newruntime = tokio::runtime::Runtime::new().unwrap();
    newruntime.block_on(async move {
        let entry = zip.file_oneshot(1).await.unwrap();
        let mut out = String::new();
        entry.compat().read_to_string(&mut out).await.unwrap();
        assert_eq!(out, "hewwo world\n");
    });
}

#![deny(clippy::pedantic)]

use argh::FromArgs;
use async_zip::tokio::read::fs::ZipFileReader;
use std::{
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::time::timeout;
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
    /// fork into background after starting
    #[argh(switch)]
    daemon: bool,
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

/// # Safety
/// must not be used when multiple threads exist
///
/// forking also messes with quite a few little things that may break rust's safety guarantees,
/// see `fork(2)` for an exhaustive list.
unsafe fn daemonize() {
    assert!(
        std::fs::metadata("/dev/null").is_ok(),
        "daemonization requires /dev/null"
    );

    // SAFETY: most safety concerns are alleviated by the parent exiting immediately,
    // but see above doc comment for issues not covered by that
    match unsafe { libc::fork() } {
        0 => {
            for n in 0..3 {
                // SAFETY: assuming there are no other threads that might be using them right now,
                // swapping out std{in,out,err} with /dev/null should be fine
                unsafe {
                    libc::close(n);
                    if libc::open(c"/dev/null".as_ptr().cast(), libc::O_RDWR, 0) != n {
                        libc::abort();
                    }
                }
            }
        }
        1.. => std::process::exit(0),
        -1 => panic!("failed to fork"),
        _ => unreachable!(),
    }
}

fn main() {
    let opt: Opt = argh::from_env();
    let cert = CertificateDer::pem_file_iter(&opt.cert)
        .expect("could not open certificate")
        .collect::<Result<Vec<_>, _>>()
        .expect("could not parse certificate");
    let key = PrivateKeyDer::from_pem_file(opt.key.as_ref().unwrap_or(&opt.cert))
        .expect("could not open private key");
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert, key)
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind(opt.bind).unwrap();
    listener.set_nonblocking(true).unwrap();

    println!("listening on {}", listener.local_addr().unwrap());

    if opt.daemon {
        eprintln!("forking to background, further errors will be eaten.");
        // SAFETY: the tokio runtime has not started yet, we are the only thread
        unsafe {
            daemonize();
        }
    }

    run(&opt, &acceptor, listener);
}

#[tokio::main]
async fn run(opt: &Opt, acceptor: &TlsAcceptor, listener: TcpListener) {
    let zip = ZipFileReader::new(&opt.zip)
        .await
        .expect("failed to open zip");
    let srv = Arc::new(server::Server::from_zip(zip));
    let listener = tokio::net::TcpListener::from_std(listener).unwrap();

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

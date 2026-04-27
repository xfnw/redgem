#![deny(clippy::pedantic)]
#![cfg_attr(not(any(feature = "daemon", feature = "recvfd")), forbid(unsafe_code))]

use argh::FromArgs;
use async_zip::tokio::read::fs::ZipFileReader;
use std::{
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    process::ExitCode,
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

#[cfg(feature = "recvfd")]
use std::os::unix::net::UnixListener;

mod server;
#[cfg(test)]
mod tests;

/// a gemini server served from a zip file
#[derive(Debug, FromArgs)]
#[argh(help_triggers("--help"))]
struct Opt {
    /// address to listen on
    #[argh(option, default = "\"[::]:1965\".parse().unwrap()")]
    bind: SocketAddr,
    /// unix socket to listen on and receive file descriptors from
    #[cfg(feature = "recvfd")]
    #[argh(option)]
    unix: Option<PathBuf>,
    /// fork into background after starting
    #[cfg(feature = "daemon")]
    #[argh(switch)]
    daemon: bool,
    /// zip file to serve files from.
    ///
    /// defaults to the current binary, serving files from a zip concatenated with itself
    #[argh(option)]
    zip: Option<PathBuf>,
    /// print version and exit
    #[expect(dead_code)]
    #[argh(switch)]
    version: bool,
    /// path to your tls certificate
    #[argh(positional)]
    cert: PathBuf,
    /// path to your tls private key.
    ///
    /// defaults to looking in the same file as your certificate
    #[argh(positional)]
    key: Option<PathBuf>,
}

#[cfg(feature = "daemon")]
fn num_threads() -> Result<usize, std::io::Error> {
    let tasks = std::fs::read_dir("/proc/self/task")?;
    Ok(tasks.count())
}

/// fork into background
///
/// # Safety
/// must not be used when multiple threads exist
///
/// forking also messes with quite a few little things that may break rust's safety guarantees,
/// see `fork(2)` for an exhaustive list.
#[cfg(feature = "daemon")]
unsafe fn daemonize() -> std::io::Result<()> {
    // SAFETY: most safety concerns are alleviated by the parent exiting immediately,
    // but see above doc comment for issues not covered by that
    match unsafe { libc::fork() } {
        0 => {
            if std::fs::metadata("/dev/null").is_ok() {
                eprintln!("forked into background, further errors will be eaten.");
                for n in 0..3 {
                    // SAFETY: assuming there are no other threads that might be using them right now,
                    // temporarily breaking rust's io safety rules by swapping out std{in,out,err}
                    // with /dev/null should be fine
                    unsafe {
                        libc::close(n);
                        if libc::open(c"/dev/null".as_ptr().cast(), libc::O_RDWR, 0) != n {
                            libc::abort();
                        }
                    }
                }
            } else {
                eprintln!("forked into background without closing standard streams.");
            }
            Ok(())
        }
        1.. => std::process::exit(0),
        -1 => Err(std::io::Error::last_os_error()),
        _ => unreachable!(),
    }
}

/// find the current executable
///
/// this differs from [`std::env::current_exe`] in that symlinks are returned instead of the target
/// on platforms that have procfs, since these links do not always target actual filesystem paths
fn path_self() -> Option<PathBuf> {
    macro_rules! search_proc {
        ($($proc:literal),*) => {
            $(
                if std::fs::metadata($proc).is_ok() {
                    return Some($proc.into());
                }
            )*
        }
    }

    search_proc!(
        "/proc/self/exe",
        "/proc/curproc/exe",
        "/proc/self/path/a.out"
    );

    // fallback to [`std::env::current_exe`] since some platforms do not just read a procfs link
    // skip platforms that only read args, since we do that next
    #[cfg(not(any(target_os = "aix", target_os = "vxworks", target_os = "fuchsia")))]
    if let Ok(path) = std::env::current_exe() {
        return Some(path);
    }

    let path = PathBuf::from(std::env::args().next()?);
    if path.exists() {
        return Some(path);
    }

    None
}

struct VersionWrapper(Opt);

impl argh::TopLevelCommand for VersionWrapper {}

impl FromArgs for VersionWrapper {
    fn from_args(command_name: &[&str], args: &[&str]) -> Result<Self, argh::EarlyExit> {
        if args
            .iter()
            .take_while(|&&s| s != "--")
            .any(|&s| s == "--version")
        {
            // kind of inelegant, but i could not think of an easier way to do this...
            // XXX: keep this up to date with the features in Cargo.toml
            let features: &[&str] = &[
                #[cfg(feature = "bzip2")]
                "bzip2",
                #[cfg(feature = "deflate")]
                "deflate",
                #[cfg(feature = "xz")]
                "xz",
                #[cfg(feature = "zstd")]
                "zstd",
                #[cfg(feature = "tls12")]
                "tls12",
                #[cfg(feature = "daemon")]
                "daemon",
                #[cfg(feature = "recvfd")]
                "recvfd",
            ];
            let mut output = format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            if let Some(info) = option_env!("REDGEM_VERSION_INFO") {
                output.push('-');
                output.push_str(info);
            }
            output.push_str("\nfeatures: ");
            output.push_str(&features.join(", "));
            return Err(argh::EarlyExit {
                output,
                status: Ok(()),
            });
        }
        Opt::from_args(command_name, args).map(Self)
    }
}

enum Listener {
    Tcp(TcpListener),
    #[cfg(feature = "recvfd")]
    Unix(UnixListener),
}

macro_rules! ear {
    ($exp:expr, $fmt:expr, $exit:expr $(, $($extra:tt)*)?) => {
        match $exp {
            Ok(o) => o,
            Err(e) => {
                eprint!($fmt $(, $($extra)*)?);
                eprintln!(": {e}");
                return ExitCode::from($exit);
            }
        }
    };
}

fn main() -> ExitCode {
    let opt = argh::from_env::<VersionWrapper>().0;

    let zip = {
        let Some(zip_path) = opt.zip.or_else(path_self) else {
            eprintln!("could not find path to myself. set it with the --zip option");
            return ExitCode::from(1);
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        ear!(
            runtime.block_on(async { ZipFileReader::new(&zip_path).await }),
            "could not open zip at {zip_path:?}",
            2
        )
    };
    let cert = ear!(
        ear!(
            CertificateDer::pem_file_iter(&opt.cert),
            "could not open certificate",
            3
        )
        .collect::<Result<Vec<_>, _>>(),
        "could not parse certificate",
        3
    );
    let key = ear!(
        PrivateKeyDer::from_pem_file(opt.key.as_ref().unwrap_or(&opt.cert)),
        "could not open private key",
        4
    );
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert, key)
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(config));

    #[cfg(feature = "recvfd")]
    let listener = if let Some(unix) = opt.unix {
        use std::os::unix::fs::FileTypeExt;

        // posix does not have a way to do this without being race condition-y :(
        if let Ok(meta) = std::fs::metadata(&unix)
            && meta.file_type().is_socket()
        {
            _ = std::fs::remove_file(&unix);
        }

        Listener::Unix(ear!(
            UnixListener::bind(unix),
            "could not bind unix socket",
            5
        ))
    } else {
        Listener::Tcp(ear!(
            TcpListener::bind(opt.bind),
            "could not bind tcp listener",
            5
        ))
    };
    #[cfg(not(feature = "recvfd"))]
    let listener = Listener::Tcp(ear!(
        TcpListener::bind(opt.bind),
        "could not bind tcp listener",
        5
    ));

    match &listener {
        Listener::Tcp(listener) => println!("listening on {}", listener.local_addr().unwrap()),
        #[cfg(feature = "recvfd")]
        Listener::Unix(listener) => println!("listening on {:?}", listener.local_addr().unwrap()),
    }

    #[cfg(feature = "daemon")]
    if opt.daemon {
        if let Ok(threads) = num_threads() {
            assert_eq!(threads, 1);
        }
        ear!(
            // SAFETY: the first tokio runtime has already been dropped and the new tokio runtime has
            // not started yet, we should be the only thread
            unsafe { daemonize() },
            "failed to daemonize",
            5
        );
    }

    run(zip, &acceptor, listener)
}

#[tokio::main]
async fn run(zip: ZipFileReader, acceptor: &TlsAcceptor, listener: Listener) -> ExitCode {
    let srv = Arc::new(server::Server::from_zip(zip));

    match listener {
        Listener::Tcp(listener) => handle_tcp(srv, acceptor, listener).await,
        #[cfg(feature = "recvfd")]
        Listener::Unix(listener) => handle_unix(srv, acceptor, listener).await,
    }
}

async fn handle_tcp(
    srv: Arc<server::Server>,
    acceptor: &TlsAcceptor,
    listener: TcpListener,
) -> ExitCode {
    listener.set_nonblocking(true).unwrap();
    let listener = tokio::net::TcpListener::from_std(listener).unwrap();

    loop {
        let (sock, _addr) = ear!(listener.accept().await, "failed to accept", 6);
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

#[cfg(feature = "recvfd")]
async fn handle_unix(
    srv: Arc<server::Server>,
    acceptor: &TlsAcceptor,
    listener: UnixListener,
) -> ExitCode {
    listener.set_nonblocking(true).unwrap();
    let listener = tokio::net::UnixListener::from_std(listener).unwrap();

    loop {
        let (sock, _addr) = ear!(listener.accept().await, "failed to accept", 6);
        let acceptor = acceptor.clone();
        let srv = srv.clone();

        tokio::spawn(async move {
            use asyncfd::UnixFdStream;
            use std::os::fd::FromRawFd;
            use tokio::io::AsyncReadExt;

            let Some(fd) = ({
                let Ok(sock) = sock.into_std() else {
                    return;
                };
                let Ok(mut sock) = UnixFdStream::new(sock, 1) else {
                    return;
                };
                // do a throwaway read so that we can get the fd from ancillary data.
                // calico just sends a null byte here
                _ = sock.read_u8().await;
                sock.pop_incoming_fd()
            }) else {
                return;
            };
            // SAFETY: we just received the fd so we should have exclusive access to it.
            // notably, from_raw_fd has no safety requirement on what kind of fd to give it. this is
            // good for us, since we could receive pretty much any kind of fd, and we do not have a
            // convenient way to check that it actually corresponds to a tcp connection
            let stream = unsafe { std::net::TcpStream::from_raw_fd(fd) };
            if stream.set_nonblocking(true).is_err() {
                return;
            }
            let Ok(stream) = tokio::net::TcpStream::from_std(stream) else {
                return;
            };
            let Ok(Ok(stream)) = timeout(Duration::from_secs(10), acceptor.accept(stream)).await
            else {
                return;
            };

            srv.handle_connection(stream).await;
        });
    }
}

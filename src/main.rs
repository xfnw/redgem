use clap::Parser;
use std::{net::SocketAddr, path::PathBuf};

mod request;
mod response;

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

#[derive(Debug, Eq, PartialEq, foxerror::FoxError)]
enum Error {
    HeaderTooLong,
    BadLineEndings,
    #[err(from)]
    NonUtf8(std::string::FromUtf8Error),
    #[err(from)]
    UnparseableUrl(ada_url::ParseUrlError<String>),
    NonGeminiScheme,
    Userinfo,
}

fn main() {
    let opt = Opt::parse();
}

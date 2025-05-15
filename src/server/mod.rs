use std::{
    collections::BTreeMap,
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use async_zip::tokio::read::fs::ZipFileReader;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::server::TlsStream;

mod request;
mod response;

#[derive(Debug, Eq, PartialEq, foxerror::FoxError)]
enum Error {
    HeaderTooLong,
    BadLineEndings,
    #[err(from)]
    NonUtf8(std::string::FromUtf8Error),
    #[err(from)]
    UnparseableUrl(url::ParseError),
    NonGeminiScheme,
    Userinfo,
    HasFragment,
    NotFound,
    BadEntry,
    Corrupted,
}

impl Error {
    fn bytes_append(&self, target: &mut Vec<u8>) {
        target.extend_from_slice(match self {
            Self::HeaderTooLong => b"59 header too long",
            Self::BadLineEndings => b"59 bad line endings",
            Self::NonUtf8(_) | Self::UnparseableUrl(_) => b"59 cannot parse url",
            Self::NonGeminiScheme => b"53 gemini scheme required",
            Self::Userinfo => b"59 your client leaks url userinfo! please report this",
            Self::HasFragment => b"59 your client leaks url fragments! please report this",
            Self::NotFound => b"51 not found",
            Self::BadEntry => b"40 failed to open zip entry",
            Self::Corrupted => b"40 zip entry corrupted",
        });
    }
}

pub struct Server {
    zip: ZipFileReader,
    index: BTreeMap<PathBuf, usize>,
}

impl Server {
    pub fn from_zip(zip: ZipFileReader) -> Self {
        let mut index = BTreeMap::new();

        for (i, entry) in zip.file().entries().iter().enumerate() {
            if entry.dir().unwrap() {
                continue;
            }

            let path = Path::new("/").join(OsStr::from_bytes(entry.filename().as_bytes()));

            if let Some("index.gmi") = path.file_name().and_then(OsStr::to_str) {
                let mut newpath = path.clone();
                newpath.pop();
                index.insert(newpath, i);
            }

            index.insert(path, i);
        }

        Self { zip, index }
    }

    pub async fn handle_connection(&self, stream: &mut TlsStream<TcpStream>) {
        let request = self.parse_req(stream).await;

        let response = match request {
            Ok(request) => {
                let path = Path::new("/").join(OsStr::from_bytes(request.pathname().as_slice()));
                self.get_file(&path).await
            }
            Err(e) => e.into(),
        };

        _ = stream.write_all(&response.into_bytes()).await;
    }

    async fn parse_req(
        &self,
        stream: &mut TlsStream<TcpStream>,
    ) -> Result<request::Request, Error> {
        let mut buffer = [0; 1026];
        let mut len = 0;

        loop {
            let Ok(count @ 1..) = stream.read(&mut buffer[len..]).await else {
                break;
            };
            len += count;
            if buffer[len - 1] == b'\n' {
                break;
            }
        }

        request::Request::parse(&buffer[..len])
    }

    async fn get_file(&self, path: &Path) -> response::Response {
        let Some(index) = self.index.get(path) else {
            return Error::NotFound.into();
        };
        let Ok(mut entry) = self.zip.reader_with_entry(*index).await else {
            return Error::BadEntry.into();
        };
        let mut out = Vec::new();
        if entry.read_to_end_checked(&mut out).await.is_err() {
            return Error::Corrupted.into();
        }
        let mimetype = response::MimeType::from_extension(path.extension(), None);
        response::Response::with_type(mimetype, out)
    }
}

use async_zip::{
    base::read::{WithEntry, ZipEntryReader},
    tokio::read::fs::ZipFileReader,
};
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader, copy},
    net::TcpStream,
    time::timeout,
};
use tokio_rustls::server::TlsStream;
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt};

mod request;
mod response;

#[derive(Debug, Eq, PartialEq, foxerror::FoxError)]
enum Error {
    HeaderTooLong,
    #[err(from)]
    NonUtf8(std::str::Utf8Error),
    #[err(from)]
    UnparseableUrl(url::ParseError),
    NonGeminiScheme,
    Userinfo,
    HasFragment,
    NotFound,
    BadEntry,
    Timeout,
}

impl Error {
    const fn bytes(&self) -> &'static [u8] {
        match self {
            Self::HeaderTooLong => b"59 header too long\r\n",
            Self::NonUtf8(_) | Self::UnparseableUrl(_) => b"59 cannot parse url\r\n",
            Self::NonGeminiScheme => b"53 gemini scheme required\r\n",
            Self::Userinfo => b"59 your client leaks url userinfo! please report this\r\n",
            Self::HasFragment => b"59 your client leaks url fragments! please report this\r\n",
            Self::NotFound => b"51 not found\r\n",
            Self::BadEntry => b"40 failed to open zip entry\r\n",
            Self::Timeout => b"40 timed out\r\n",
        }
    }
}

pub struct Server {
    zip: ZipFileReader,
    index: BTreeMap<PathBuf, (usize, bool)>,
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
                index.insert(newpath, (i, true));
            }

            index.insert(path, (i, false));
        }

        Self { zip, index }
    }

    pub async fn handle_connection(&self, mut stream: TlsStream<TcpStream>) {
        let Ok(request) = timeout(Duration::from_secs(30), self.parse_req(&mut stream)).await
        else {
            _ = timeout(
                Duration::from_secs(30),
                send_response::<&[u8]>(stream, Error::Timeout.into()),
            )
            .await;
            return;
        };

        let response = match request {
            Ok(request) => {
                let path = Path::new("/").join(OsStr::from_bytes(request.pathname().as_slice()));
                self.get_file(&path).await
            }
            Err(e) => e.into(),
        };

        _ = timeout(Duration::from_secs(300), send_response(stream, response)).await;
    }

    async fn parse_req(
        &self,
        stream: &mut TlsStream<TcpStream>,
    ) -> Result<request::Request, Error> {
        let mut buffer = [0; 1026];
        let mut len = 0;

        loop {
            let Ok(count @ 1..) = stream.read(&mut buffer[len..]).await else {
                return Err(Error::HeaderTooLong);
            };
            len += count;
            if buffer[..len].ends_with(b"\r\n") {
                break;
            }
        }

        request::Request::parse(&buffer[..len - 2])
    }

    async fn get_file(
        &self,
        path: &Path,
    ) -> response::Response<Compat<ZipEntryReader<'_, Compat<BufReader<File>>, WithEntry<'_>>>>
    {
        let Some(&(id, is_index)) = self.index.get(path) else {
            return Error::NotFound.into();
        };
        let Ok(entry) = self.zip.reader_with_entry(id).await else {
            return Error::BadEntry.into();
        };
        let mimetype =
            response::MimeType::from_extension(if is_index { None } else { path.extension() });
        response::Response::with_type(mimetype, entry.compat())
    }
}

async fn send_response<R>(mut stream: TlsStream<TcpStream>, response: response::Response<R>)
where
    R: AsyncRead + Unpin,
{
    if copy(&mut response.into_read(), &mut stream).await.is_ok() {
        _ = stream.shutdown().await;
    }
}

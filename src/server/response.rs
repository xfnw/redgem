use super::{Error, request::Request};
use pin_project_lite::pin_project;
use std::{
    ffi::OsStr,
    io::Cursor,
    pin::Pin,
    task::{Context, Poll, ready},
};
use tokio::io::{AsyncRead, ReadBuf};

/// the file type for a successful [`Response`]
#[derive(Debug)]
pub struct MimeType {
    domtype: &'static str,
    subtype: &'static str,
}

impl MimeType {
    /// guess the type using a file extension
    pub fn from_extension(ext: Option<&OsStr>) -> Self {
        let (domtype, subtype) = match ext
            .and_then(OsStr::to_str)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("c" | "cc" | "cpp" | "cxx" | "h" | "hh" | "hpp" | "hxx" | "rs") => ("text", "x-c"),
            Some("css") => ("text", "css"),
            Some("csv") => ("text", "csv"),
            Some("diff") => ("text", "x-diff"),
            Some("gif") => ("image", "gif"),
            Some("gmi" | "gemini") | None => ("text", "gemini"),
            Some("go") => ("text", "x-go"),
            Some("gpub") => ("application", "gpub+zip"),
            Some("html" | "htm") => ("text", "html"),
            Some("jpeg" | "jpg") => ("image", "jpeg"),
            Some("js" | "mjs") => ("text", "javascript"),
            Some("json") => ("application", "json"),
            Some("m3u") => ("audio", "x-mpegurl"),
            Some("md" | "mdwn" | "markdown") => ("text", "markdown"),
            Some("mp3") => ("audio", "mpeg"),
            Some("mp4") => ("video", "mp4"),
            Some("ogg") => ("application", "ogg"),
            Some("patch") => ("text", "x-patch"),
            Some("pdf") => ("application", "pdf"),
            Some("png") => ("image", "png"),
            Some("py") => ("text", "x-script.python"),
            Some("sh") => ("text", "x-shellscript"),
            Some("svg") => ("image", "svg+xml"),
            Some("torrent") => ("application", "x-bittorrent"),
            Some("tsv") => ("text", "tab-separated-values"),
            Some(
                "txt" | "asc" | "conf" | "el" | "log" | "lua" | "nix" | "org" | "pm" | "tal"
                | "text" | "toml" | "vf" | "yml" | "yaml",
            ) => ("text", "plain"),
            Some("vcf" | "vcard") => ("text", "vcard"),
            Some("wasm") => ("application", "wasm"),
            Some("wav") => ("audio", "x-wav"),
            Some("webm") => ("video", "webm"),
            Some("webp") => ("image", "webp"),
            Some("xml" | "xsl") => ("text", "xml"),
            Some("zip") => ("application", "zip"),
            Some("zstd" | "zst") => ("application", "zstd"),
            Some(_) => ("application", "octet-stream"),
        };

        Self { domtype, subtype }
    }

    fn bytes_append(&self, target: &mut Vec<u8>) {
        target.extend_from_slice(self.domtype.as_bytes());
        target.push(b'/');
        target.extend_from_slice(self.subtype.as_bytes());
    }
}

/// a gemini protocol response
#[non_exhaustive]
pub enum Response<B> {
    Success { mimetype: MimeType, body: B },
    Failure { kind: Error },
    PermanentRedirect { to: Request },
}

impl<B> Response<B> {
    /// create a successful response
    pub const fn with_type(mimetype: MimeType, body: B) -> Self {
        Self::Success { mimetype, body }
    }

    /// create a permanent redirect response
    pub const fn permanent_redirect(to: Request) -> Self {
        Self::PermanentRedirect { to }
    }

    /// turn the response into a tokio [`AsyncRead`]
    pub fn into_read(self) -> OptionalChain<Cursor<Vec<u8>>, B> {
        match self {
            Self::Success { mimetype, body } => {
                let mut header = b"20 ".to_vec();
                mimetype.bytes_append(&mut header);
                header.extend_from_slice(b"\r\n");
                OptionalChain::chain(Cursor::new(header), body)
            }
            Self::Failure { kind } => OptionalChain::single(Cursor::new(kind.bytes().to_vec())),
            Self::PermanentRedirect { to } => {
                let mut header = b"31 ".to_vec();
                header.extend_from_slice(to.as_str().as_bytes());
                header.extend_from_slice(b"\r\n");
                OptionalChain::single(Cursor::new(header))
            }
        }
    }
}

impl<B> From<Error> for Response<B> {
    fn from(err: Error) -> Self {
        Self::Failure { kind: err }
    }
}

pin_project! {
    /// tokio's Chain but optional
    #[project = OptionalChainProject]
    #[must_use = "you should read this"]
    pub enum OptionalChain<T, U> {
        Chain {
            #[pin]
            first: T,
            #[pin]
            second: U,
            done_first: bool,
        },
        Single {
            #[pin]
            first: T,
        },
    }
}

impl<T, U> OptionalChain<T, U> {
    pub const fn chain(first: T, second: U) -> Self {
        Self::Chain {
            first,
            second,
            done_first: false,
        }
    }

    pub const fn single(first: T) -> Self {
        Self::Single { first }
    }
}

impl<T, U> AsyncRead for OptionalChain<T, U>
where
    T: AsyncRead,
    U: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.project() {
            OptionalChainProject::Chain {
                first,
                second,
                done_first,
            } => {
                if !*done_first {
                    let rem = buf.remaining();
                    ready!(first.poll_read(cx, buf))?;
                    if buf.remaining() == rem {
                        *done_first = true;
                    } else {
                        return Poll::Ready(Ok(()));
                    }
                }

                second.poll_read(cx, buf)
            }
            OptionalChainProject::Single { first } => first.poll_read(cx, buf),
        }
    }
}

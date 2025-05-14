use std::ffi::OsStr;

#[derive(Debug)]
pub struct MimeType {
    domtype: &'static str,
    subtype: &'static str,
    charset: Option<String>,
}

impl MimeType {
    pub fn from_extension(ext: Option<&OsStr>, charset: Option<String>) -> Self {
        let (domtype, subtype) = match ext.and_then(OsStr::to_str) {
            Some("c") | Some("rs") => ("text", "x-c"),
            Some("css") => ("text", "css"),
            Some("gif") => ("image", "gif"),
            Some("gmi") | None => ("text", "gemini"),
            Some("html") | Some("htm") => ("text", "html"),
            Some("jpeg") | Some("jpg") => ("image", "jpeg"),
            Some("js") => ("application", "x-javascript"),
            Some("json") => ("application", "json"),
            Some("m3u") => ("audio", "x-mpegurl"),
            Some("mp3") => ("audio", "mpeg"),
            Some("mp4") => ("video", "mp4"),
            Some("ogg") => ("application", "ogg"),
            Some("png") => ("image", "png"),
            Some("py") => ("text", "x-script.python"),
            Some("sh") => ("text", "x-shellscript"),
            Some("svg") => ("image", "svg+xml"),
            Some("torrent") => ("application", "x-bittorrent"),
            Some("txt") | Some("tal") | Some("vf") => ("text", "plain"),
            Some("wasm") => ("application", "wasm"),
            Some("wav") => ("audio", "x-wav"),
            Some("webm") => ("video", "webm"),
            Some("webp") => ("image", "webp"),
            Some("xml") | Some("xsl") => ("text", "xml"),
            Some("zip") => ("application", "zip"),
            Some("zstd") | Some("zst") => ("application", "zstd"),
            Some(_) => ("application", "octet-stream"),
        };

        Self {
            domtype,
            subtype,
            charset,
        }
    }

    fn bytes_append(&self, target: &mut Vec<u8>) {
        target.extend_from_slice(self.domtype.as_bytes());
        target.push(b'/');
        target.extend_from_slice(self.subtype.as_bytes());

        if let Some(charset) = &self.charset {
            target.extend_from_slice(b"; charset=");
            target.extend_from_slice(charset.as_bytes());
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum PermKind {
    NotFound,
    BadRequest,
}

impl PermKind {
    fn bytes_append(&self, target: &mut Vec<u8>) {
        target.extend_from_slice(match self {
            Self::NotFound => b"51 not found",
            Self::BadRequest => b"59 bad request",
        });
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Response {
    Success { mimetype: MimeType, body: Vec<u8> },
    PermFail { kind: PermKind },
}

impl Response {
    pub fn with_type(mimetype: MimeType, body: Vec<u8>) -> Self {
        Self::Success { mimetype, body }
    }

    pub fn not_found() -> Self {
        Self::PermFail {
            kind: PermKind::NotFound,
        }
    }

    pub fn bad_request() -> Self {
        Self::PermFail {
            kind: PermKind::BadRequest,
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        let mut out = Vec::new();

        match self {
            Self::Success { mimetype, mut body } => {
                out.extend_from_slice(b"20 ");
                mimetype.bytes_append(&mut out);
                out.extend_from_slice(b"\r\n");
                out.append(&mut body);
            }
            Self::PermFail { kind } => {
                kind.bytes_append(&mut out);
                out.extend_from_slice(b"\r\n");
            }
        }

        out
    }
}

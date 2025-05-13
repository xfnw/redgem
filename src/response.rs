#[derive(Debug)]
pub struct MimeType {
    domtype: &'static str,
    subtype: &'static str,
    charset: Option<String>,
}

impl MimeType {
    pub fn from_extension(ext: &str, charset: Option<String>) -> Self {
        let (domtype, subtype) = match ext {
            "c" | "rs" => ("text", "x-c"),
            "css" => ("text", "css"),
            "gif" => ("image", "gif"),
            "gmi" => ("text", "gemini"),
            "html" | "htm" => ("text", "html"),
            "jpeg" | "jpg" => ("image", "jpeg"),
            "js" => ("application", "x-javascript"),
            "json" => ("application", "json"),
            "m3u" => ("audio", "x-mpegurl"),
            "mp3" => ("audio", "mpeg"),
            "mp4" => ("video", "mp4"),
            "ogg" => ("application", "ogg"),
            "png" => ("image", "png"),
            "py" => ("text", "x-script.python"),
            "sh" => ("text", "x-shellscript"),
            "svg" => ("image", "svg+xml"),
            "torrent" => ("application", "x-bittorrent"),
            "txt" | "tal" | "vf" => ("text", "plain"),
            "wasm" => ("application", "wasm"),
            "wav" => ("audio", "x-wav"),
            "webm" => ("video", "webm"),
            "webp" => ("image", "webp"),
            "xml" | "xsl" => ("text", "xml"),
            "zip" => ("application", "zip"),
            "zstd" | "zst" => ("application", "zstd"),
            _ => ("application", "octet-stream"),
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

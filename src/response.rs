#[derive(Debug)]
pub struct MimeType {
    domtype: String,
    subtype: String,
    charset: Option<String>,
}

impl MimeType {
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

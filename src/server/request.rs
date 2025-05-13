use super::Error;
use ada_url::Url;

#[derive(Debug)]
pub struct Request(Url);

impl Request {
    pub fn parse(inp: &[u8]) -> Result<Self, Error> {
        let s = match inp.iter().position(|&b| b == b'\r') {
            Some(1025..) => {
                return Err(Error::HeaderTooLong);
            }
            Some(len) => {
                if let Some(b'\n') = inp.get(len + 1) {
                    String::from_utf8(inp[..len].to_vec())?
                } else {
                    return Err(Error::BadLineEndings);
                }
            }
            None => {
                return Err(Error::BadLineEndings);
            }
        };

        let u = Url::parse(s, None)?;

        if u.protocol() != "gemini:" {
            return Err(Error::NonGeminiScheme);
        }

        if u.has_credentials() {
            return Err(Error::Userinfo);
        }

        Ok(Self(u))
    }

    #[inline]
    pub fn pathname(&self) -> &str {
        self.0.pathname()
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, Request};

    #[test]
    fn length() {
        let hhhh = [b'h'; 1025];
        let eol = b"\r\n";

        let mut long = hhhh.to_vec();
        long.extend_from_slice(eol);
        assert_eq!(Request::parse(&long).unwrap_err(), Error::HeaderTooLong);

        let mut short = hhhh[..1024].to_vec();
        short.extend_from_slice(eol);
        assert_ne!(Request::parse(&short).unwrap_err(), Error::HeaderTooLong);
    }

    #[test]
    fn parse_pathname() {
        assert_eq!(
            Request::parse(b"gemini://example.com/meow\r\n")
                .unwrap()
                .pathname(),
            "/meow"
        );
    }
}

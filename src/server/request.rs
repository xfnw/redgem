use super::Error;
use fluent_uri::{Uri, component::Scheme, encoding::Decode};

#[derive(Debug)]
pub struct Request(Uri<String>);

impl Request {
    pub fn parse(inp: &[u8]) -> Result<Self, Error> {
        let u = Uri::parse(str::from_utf8(inp)?.to_string()).map_err(|_| Error::UnparseableUri)?;

        if u.scheme() != const { Scheme::new_or_panic("gemini") } {
            return Err(Error::NonGeminiScheme);
        }

        if let Some(authority) = u.authority() {
            if authority.has_userinfo() {
                return Err(Error::Userinfo);
            }
        } else {
            return Err(Error::NoAuthority);
        }

        if u.has_fragment() {
            return Err(Error::HasFragment);
        }

        Ok(Self(u))
    }

    #[inline]
    pub fn pathname(&self) -> Vec<u8> {
        match self.0.path().decode() {
            Decode::Borrowed(b) => b.as_bytes().to_vec(),
            Decode::Owned(v) => v,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Request;

    #[test]
    fn parse_pathname() {
        assert_eq!(
            Request::parse(b"gemini://example.com/meow")
                .unwrap()
                .pathname(),
            b"/meow"
        );
    }
}

use super::Error;
use fluent_uri::{Uri, component::Scheme, encoding::Decode};

/// a parsed gemini request
#[derive(Debug)]
pub struct Request(Uri<String>);

impl Request {
    /// parse a gemini request from bytes
    ///
    /// this expects line endings to already have been removed
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

    /// get the path from a request
    #[inline]
    pub fn pathname(&self) -> Decode<'_> {
        self.0.path().decode()
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
                .pathname()
                .as_bytes(),
            b"/meow"
        );
    }
}

use super::Error;
use fluent_uri::{Uri, component::Scheme, pct_enc::Decode};

/// a parsed gemini request
#[derive(Debug)]
pub struct Request(Uri<String>);

impl Request {
    /// parse a gemini request from bytes
    ///
    /// this expects the trailing line ending to already have been removed, and will return an
    /// error if the input contains a line ending
    pub fn parse(inp: &[u8], expect_host: Option<&str>) -> Result<Self, Error> {
        let u = Uri::parse(str::from_utf8(inp)?.to_string()).map_err(|_| Error::UnparseableUri)?;

        if u.scheme() != const { Scheme::new_or_panic("gemini") } {
            return Err(Error::NonGeminiScheme);
        }

        if let Some(authority) = u.authority() {
            if expect_host.is_some_and(|h| !h.eq_ignore_ascii_case(authority.host())) {
                return Err(Error::SniMismatch);
            }
            if authority.has_userinfo() {
                return Err(Error::Userinfo);
            }
        } else {
            return Err(Error::NoAuthority);
        }

        if u.has_query() {
            return Err(Error::HasQuery);
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

    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// create a new request with a `/` added to the end of the path.
    ///
    /// the result will be nonsensical if it already has a trailing `/`
    pub fn with_trailing(&self) -> Result<Self, Error> {
        let mut path = self.0.path().to_owned();
        path.push('/');

        let uri = Uri::builder()
            .scheme(self.0.scheme())
            .authority(self.0.authority().expect("Request must have authority"))
            .path(&path)
            .build()
            .map_err(|_| Error::UriBuild)?;

        Ok(Self(uri))
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, Request};

    macro_rules! all_err {
        (($($req:literal),*), $err:expr) => {
            $(
                assert_eq!(Request::parse($req, None).unwrap_err(), $err);
            )*
        }
    }

    #[test]
    fn no_newlines() {
        all_err!(
            (
                b"gem\rini://example.com/meow",
                b"gem\nini://example.com/meow",
                b"gemini://exam\rple.com/meow",
                b"gemini://exam\nple.com/meow",
                b"gemini://example.com/me\row",
                b"gemini://example.com/me\now"
            ),
            Error::UnparseableUri
        );
    }

    #[test]
    fn parse_pathname() {
        assert_eq!(
            Request::parse(b"gemini://example.com:1234/meow", Some("Example.com"))
                .unwrap()
                .pathname()
                .to_bytes().as_ref(),
            b"/meow"
        );
    }

    #[test]
    fn bad_host() {
        assert_eq!(
            Request::parse(b"gemini://geminiprotocol.net", Some("example.com")).unwrap_err(),
            Error::SniMismatch
        );
    }
}

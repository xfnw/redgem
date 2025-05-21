use super::Error;
use percent_encoding::percent_decode_str;
use url::Url;

#[derive(Debug)]
pub struct Request(Url);

impl Request {
    pub fn parse(inp: &[u8]) -> Result<Self, Error> {
        let u = Url::parse(str::from_utf8(inp)?)?;

        if u.scheme() != "gemini" {
            return Err(Error::NonGeminiScheme);
        }

        if u.username() != "" || u.password().is_some() {
            return Err(Error::Userinfo);
        }

        if u.fragment().is_some() {
            return Err(Error::HasFragment);
        }

        Ok(Self(u))
    }

    #[inline]
    pub fn pathname(&self) -> Vec<u8> {
        percent_decode_str(self.0.path()).collect()
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

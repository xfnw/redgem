mod request;
mod response;

#[derive(Debug, Eq, PartialEq, foxerror::FoxError)]
enum Error {
    HeaderTooLong,
    BadLineEndings,
    #[err(from)]
    NonUtf8(std::string::FromUtf8Error),
    #[err(from)]
    UnparseableUrl(ada_url::ParseUrlError<String>),
    NonGeminiScheme,
    Userinfo,
}

fn main() {}

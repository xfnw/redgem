use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::server::TlsStream;

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

#[derive(Debug)]
pub struct Server {}

impl Server {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn handle_connection(&self, mut stream: &mut TlsStream<TcpStream>) {
        let request = self.parse_req(&mut stream).await;

        let response = if let Ok(request) = request {
            todo!()
        } else {
            response::Response::bad_request()
        };

        _ = stream.write_all(&response.into_bytes()).await;
    }

    async fn parse_req(
        &self,
        stream: &mut TlsStream<TcpStream>,
    ) -> Result<request::Request, Error> {
        let mut buffer = [0; 1026];
        let mut len = 0;

        loop {
            let Ok(count @ 1..) = stream.read(&mut buffer[len..]).await else {
                break;
            };
            len += count;
            if buffer[len - 1] == b'\n' {
                break;
            }
        }

        request::Request::parse(&buffer[..len])
    }
}

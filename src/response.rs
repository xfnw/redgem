use ada_url::Url;

#[derive(Debug)]
pub struct MimeType {}

#[derive(Debug)]
pub enum Response {
    Input { prompt: String },
    Success { mimetype: MimeType, body: Vec<u8> },
    Redirect { url: Url },
    TempFail { errormsg: String },
    PermFail { errormsg: String },
    Auth { errormsg: String },
}

#[derive(Debug)]
pub struct HTTPRequestEvent {
    pub type_: String,
    pub body: Vec<u8>,
    pub more_body: bool,
}

impl HTTPRequestEvent {
    pub fn new(body: Vec<u8>, more_body: bool) -> Self {
        Self {
            type_: "http.request".into(),
            body,
            more_body,
        }
    }
}

#[derive(Debug)]
pub struct HTTPResponseStartEvent {
    pub type_: String,
    pub status: u16,
    pub headers: Vec<(Vec<u8>, Vec<u8>)>,
    pub trailers: bool,
}

impl HTTPResponseStartEvent {
    pub fn new(status: u16, headers: Vec<(Vec<u8>, Vec<u8>)>, trailers: bool) -> Self {
        Self {
            type_: "http.response.start".into(),
            status,
            headers,
            trailers,
        }
    }
}

#[derive(Debug)]
pub struct HTTPResonseBodyEvent {
    pub type_: String,
    pub body: Vec<u8>,
    pub more_body: bool,
}

impl HTTPResonseBodyEvent {
    pub fn new(body: Vec<u8>, more_body: bool) -> Self {
        Self {
            type_: "http.response.body".into(),
            body,
            more_body,
        }
    }
}

#[derive(Debug)]
pub struct HTTPDisconnectEvent {
    pub type_: String,
}

impl HTTPDisconnectEvent {
    pub fn new() -> Self {
        Self { type_: "http.disconnect".into() }
    }
}
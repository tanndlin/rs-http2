use std::fmt::Display;

use crate::types::Header;

pub enum StatusCode {
    Ok,
    NotFound,
    BadRequest,
    MethodNotAllowed,
    InteralServerError,
}

impl Display for StatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatusCode::Ok => write!(f, "200 OK"),
            StatusCode::NotFound => write!(f, "404 Not Found"),
            StatusCode::BadRequest => write!(f, "400 Bad Request"),
            StatusCode::MethodNotAllowed => write!(f, "405 Method Not Allowed"),
            StatusCode::InteralServerError => write!(f, "500 Internal Server Error"),
        }
    }
}

pub struct Response {
    status_code: StatusCode,
    headers: Vec<Header>,
    body: Vec<u8>,
}

impl Response {
    pub fn bad_request() -> Self {
        ResponseBuilder::new()
            .status_code(StatusCode::BadRequest)
            .build()
    }

    pub fn not_found() -> Self {
        ResponseBuilder::new()
            .status_code(StatusCode::NotFound)
            .build()
    }

    pub fn method_not_allowed() -> Self {
        ResponseBuilder::new()
            .status_code(StatusCode::MethodNotAllowed)
            .build()
    }

    pub fn internal_server_error() -> Self {
        ResponseBuilder::new()
            .status_code(StatusCode::InteralServerError)
            .build()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = vec![];
        buffer.extend_from_slice(format!("HTTP/1.1 {}\r\n", self.status_code).as_bytes());
        for header in &self.headers {
            buffer.extend_from_slice(format!("{}: {}\r\n", header.name, header.value).as_bytes());
        }

        buffer.extend_from_slice(b"\r\n");
        buffer.extend_from_slice(&self.body);
        buffer
    }
}

pub struct ResponseBuilder {
    status_code: StatusCode,
    headers: Vec<Header>,
    body: Option<Vec<u8>>,
}

impl ResponseBuilder {
    pub fn new() -> Self {
        ResponseBuilder {
            status_code: StatusCode::InteralServerError,
            headers: vec![],
            body: None,
        }
    }

    pub fn status_code(mut self, code: StatusCode) -> Self {
        self.status_code = code;
        self
    }

    pub fn header(mut self, name: String, value: String) -> Self {
        self.headers.push(Header { name, value });
        self
    }

    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }

    pub fn build(self) -> Response {
        let mut headers = self.headers;
        if !headers.iter().any(|h| h.name == "Content-Length") {
            let length = match &self.body {
                Some(b) => b.len(),
                None => 0,
            };
            headers.push(Header {
                name: "Content-Length".to_string(),
                value: length.to_string(),
            });
        }

        Response {
            status_code: self.status_code,
            headers,
            body: self.body.unwrap_or_default(),
        }
    }
}

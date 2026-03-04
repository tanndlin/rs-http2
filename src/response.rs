use std::{collections::HashMap, fmt::Display};

#[derive(Debug)]
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

impl StatusCode {
    pub fn to_code(&self) -> u32 {
        match self {
            StatusCode::Ok => 200,
            StatusCode::NotFound => 404,
            StatusCode::BadRequest => 400,
            StatusCode::MethodNotAllowed => 405,
            StatusCode::InteralServerError => 500,
        }
    }
}

#[derive(Debug)]
pub struct Response {
    pub status_code: StatusCode,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub stream_id: u32,
}

impl Response {
    pub fn bad_request(stream_id: u32) -> Self {
        ResponseBuilder::new()
            .status_code(StatusCode::BadRequest)
            .stream_id(stream_id)
            .build()
    }

    pub fn not_found(stream_id: u32) -> Self {
        ResponseBuilder::new()
            .status_code(StatusCode::NotFound)
            .stream_id(stream_id)
            .build()
    }

    pub fn method_not_allowed(stream_id: u32) -> Self {
        ResponseBuilder::new()
            .status_code(StatusCode::MethodNotAllowed)
            .stream_id(stream_id)
            .build()
    }

    pub fn internal_server_error(stream_id: u32) -> Self {
        ResponseBuilder::new()
            .status_code(StatusCode::InteralServerError)
            .stream_id(stream_id)
            .build()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = vec![];
        buffer.extend_from_slice(format!("HTTP/1.1 {}\r\n", self.status_code).as_bytes());
        for (name, value) in &self.headers {
            buffer.extend_from_slice(format!("{}: {}\r\n", name, value).as_bytes());
        }

        buffer.extend_from_slice(b"\r\n");
        buffer.extend_from_slice(&self.body);
        buffer
    }
}

pub struct ResponseBuilder {
    status_code: StatusCode,
    headers: HashMap<String, String>,
    body: Option<Vec<u8>>,
    stream_id: u32,
}

macro_rules! add_if_missing {
    ($name:expr, $headers:expr, $callback:expr) => {
        if !$headers.contains_key($name) {
            $headers.insert($name.to_string(), $callback());
        }
    };
}

impl ResponseBuilder {
    pub fn new() -> Self {
        ResponseBuilder {
            status_code: StatusCode::InteralServerError,
            headers: HashMap::new(),
            body: None,
            stream_id: 0,
        }
    }

    pub fn status_code(mut self, code: StatusCode) -> Self {
        self.status_code = code;
        self
    }

    pub fn header(mut self, name: String, value: String) -> Self {
        self.headers.insert(name, value);
        self
    }

    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }

    pub fn stream_id(mut self, stream_id: u32) -> Self {
        self.stream_id = stream_id;
        self
    }

    pub fn build(self) -> Response {
        let mut headers = self.headers;

        add_if_missing!("Date", headers, || chrono::Utc::now().to_rfc2822());
        add_if_missing!("Content-Length", headers, || self
            .body
            .as_ref()
            .map_or(0, |b| b.len())
            .to_string());

        Response {
            status_code: self.status_code,
            headers,
            body: self.body.unwrap_or_default(),
            stream_id: self.stream_id,
        }
    }
}

use std::str::FromStr;

use crate::types::Header;

#[allow(clippy::upper_case_acronyms)]
pub enum Method {
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
}

impl FromStr for Method {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GET" => Ok(Method::GET),
            "POST" => Ok(Method::POST),
            "PUT" => Ok(Method::PUT),
            "DELETE" => Ok(Method::DELETE),
            "HEAD" => Ok(Method::HEAD),
            _ => Err("Unrecognized method"),
        }
    }
}

pub struct Request {
    pub method: Method,
    pub path: String,
    pub headers: Vec<Header>,
    pub body: String,
}

impl Request {
    pub fn from_bytes(bytes: [u8; 1024]) -> Result<Self, String> {
        let request_str = String::from_utf8_lossy(&bytes);
        let mut lines = request_str.lines();

        // Get method and path
        let first_line = lines.next().ok_or("Empty request")?;
        let mut first_line_parts = first_line.split_whitespace();
        let method = first_line_parts.next().ok_or("Missing Method")?.to_string();
        let path = first_line_parts.next().ok_or("Missing path")?.to_string();
        let version = first_line_parts
            .next()
            .ok_or("Missing version")?
            .to_string();

        if version != "HTTP/1.1" {
            return Err("Unsupported HTTP version".to_string());
        }

        // Get headers
        let mut headers = Vec::new();
        for line in lines.by_ref() {
            if line.is_empty() {
                break;
            }
            let mut parts = line.splitn(2, ": ");
            let name = parts.next().ok_or("header missing name")?.to_string();
            let value = parts.next().ok_or("Header missing value")?.to_string();
            headers.push(Header { name, value });
        }

        // Get body
        let body = lines.collect::<Vec<&str>>().join("\n");

        Ok(Self {
            method: Method::from_str(&method)?,
            path,
            headers,
            body,
        })
    }

    pub fn get_header(&self, name: &str) -> Option<&Header> {
        self.headers.iter().find(|h| h.name == name)
    }
}

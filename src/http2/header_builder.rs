use std::{collections::HashMap, str::FromStr};

use hpack::Decoder;

use crate::http2::{
    error::{HTTP2Error, HTTP2ErrorCode, StreamError},
    psuedo_headers::PsuedoHeader,
};

#[derive(Debug)]
pub struct HeaderBuilder {
    data: Vec<u8>,
}

impl HeaderBuilder {
    pub fn new_fragment(&mut self, buf: Vec<u8>) {
        self.data.extend(buf);
    }

    pub fn build(
        &mut self,
        decoder: &mut Decoder,
        stream_id: u32,
    ) -> Result<HashMap<String, String>, HTTP2Error> {
        let decoded_headers = decoder
            .decode(&self.data)
            .map_err(|_| HTTP2Error::Connection(HTTP2ErrorCode::CompressionError))?;
        self.data.clear();

        let mut headers: HashMap<String, String> = HashMap::new();
        let mut seen_regular_header = false;
        for (name, value) in decoded_headers {
            let name = String::from_utf8_lossy(&name);
            let value = String::from_utf8_lossy(&value);

            // No duplicate pseudo headers allowed, and pseudo headers must come before regular headers
            if name.starts_with(':') {
                if headers.contains_key(&name.to_string()) || seen_regular_header {
                    return Err(HTTP2Error::Stream(StreamError {
                        stream_id,
                        error_code: HTTP2ErrorCode::ProtocolError,
                    }));
                }
            } else {
                seen_regular_header = true;
            }

            headers.insert(name.to_string(), value.to_string());
        }

        // Check for uppercase letters in header names
        if headers
            .keys()
            .any(|h| h.chars().any(|c| c.is_ascii_uppercase()))
        {
            return Err(HTTP2Error::Stream(StreamError {
                stream_id,
                error_code: HTTP2ErrorCode::ProtocolError,
            }));
        }

        // Check for presence of pseudo headers and that they are valid
        if headers
            .iter()
            .filter(|h| h.0.starts_with(':'))
            .any(|(k, v)| !is_req_psuedo_header_valid(k, v))
        {
            return Err(HTTP2Error::Stream(StreamError {
                stream_id,
                error_code: HTTP2ErrorCode::ProtocolError,
            }));
        }

        // Scheme and method must be present in requests
        if !headers.contains_key(":scheme") || !headers.contains_key(":method") {
            return Err(HTTP2Error::Stream(StreamError {
                stream_id,
                error_code: HTTP2ErrorCode::ProtocolError,
            }));
        }

        // Must not contain connection-specific headers
        for (key, value) in &headers {
            match key.as_str() {
                "connection" | "proxy-connection" | "keep-alive" | "transfer-encoding"
                | "upgrade" => {
                    return Err(HTTP2Error::Stream(StreamError {
                        stream_id,
                        error_code: HTTP2ErrorCode::ProtocolError,
                    }));
                }
                "te" => {
                    if value.to_lowercase() != "trailers" {
                        return Err(HTTP2Error::Stream(StreamError {
                            stream_id,
                            error_code: HTTP2ErrorCode::ProtocolError,
                        }));
                    }
                }
                _ => (),
            }
        }

        Ok(headers)
    }

    pub fn new() -> Self {
        Self { data: vec![] }
    }

    pub fn waiting_for_continuation(&self) -> bool {
        !self.data.is_empty()
    }
}

fn is_req_psuedo_header_valid(key: &str, value: &str) -> bool {
    let Ok(header) = PsuedoHeader::from_str(key) else {
        return false;
    };

    // Cant be :status
    if let PsuedoHeader::Status = header {
        return false;
    }

    // Value must not be empty
    if value.is_empty() {
        return false;
    }

    true
}

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
        dbg!("Decoding");
        let decoded_headers = decoder
            .decode(&self.data)
            .map_err(|_| HTTP2Error::Connection(HTTP2ErrorCode::CompressionError))?;
        self.data.clear();
        // dbg!(&decoded_headers);

        let mut headers: HashMap<String, String> = HashMap::new();
        for (name, value) in decoded_headers {
            let name = String::from_utf8_lossy(&name);
            let value = String::from_utf8_lossy(&value);

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
            .keys()
            .filter(|h| h.starts_with(':'))
            .any(|h| PsuedoHeader::from_str(h).is_err() || *h == PsuedoHeader::Status.to_string())
        {
            return Err(HTTP2Error::Stream(StreamError {
                stream_id,
                error_code: HTTP2ErrorCode::ProtocolError,
            }));
        }

        // Make sure all pseudo headers come before regular headers
        let mut seen_regular_header = false;
        for header_name in headers.keys() {
            if header_name.starts_with(':') {
                if seen_regular_header {
                    return Err(HTTP2Error::Stream(StreamError {
                        stream_id,
                        error_code: HTTP2ErrorCode::ProtocolError,
                    }));
                }
            } else {
                seen_regular_header = true;
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

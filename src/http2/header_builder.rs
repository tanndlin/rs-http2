use std::collections::HashMap;

use hpack::Decoder;

use crate::http2::error::{HTTP2Error, HTTP2ErrorCode};

#[derive(Debug)]
pub struct HeaderBuilder {
    data: Vec<u8>,
}

impl HeaderBuilder {
    pub fn new_fragment(&mut self, buf: Vec<u8>) {
        self.data.extend(buf);
    }

    pub fn build(&mut self, decoder: &mut Decoder) -> Result<HashMap<String, String>, HTTP2Error> {
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

        Ok(headers)
    }

    pub fn new() -> Self {
        Self { data: vec![] }
    }

    pub fn waiting_for_continuation(&self) -> bool {
        !self.data.is_empty()
    }
}

use std::collections::HashMap;

use hpack::Decoder;

pub struct HeaderBuilder {
    data: Vec<u8>,
}

impl HeaderBuilder {
    pub fn new_fragment(&mut self, buf: &[u8]) {
        self.data.extend_from_slice(buf);
    }

    pub fn build(&mut self, decoder: &mut Decoder) -> Result<HashMap<String, String>, String> {
        dbg!("Decoding");
        let decoded_headers = decoder
            .decode(&self.data)
            .map_err(|e| format!("Error decoding compressed headers: {:?}", e))?;
        self.data.clear();
        dbg!(&decoded_headers);

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
}

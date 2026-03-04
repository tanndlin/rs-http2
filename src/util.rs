use std::{collections::HashMap, str::FromStr};

use crate::{
    http2::{
        connection_state::ConnectionState,
        frames::{continuation_frame::ContinuationFrame, headers_frame::HeadersFrame},
        gc_buffer::GCBuffer,
    },
    request::{Method, Request},
};

pub fn u32_from_3_bytes(buf: &[u8; 3]) -> u32 {
    (buf[0] as u32) << 16 | (buf[1] as u32) << 8 | (buf[2] as u32)
}

fn decode_headers(
    stream_id: u32,
    compressed_headers: &[u8],
    state: &mut ConnectionState,
) -> Result<Request, String> {
    dbg!("Decoding");
    let decoded_headers = state
        .decoder
        .decode(&compressed_headers)
        .map_err(|e| format!("Error decoding compressed headers: {:?}", e))?;

    dbg!(&decoded_headers);

    let mut headers: HashMap<String, String> = HashMap::new();
    for (name, value) in decoded_headers {
        let name = String::from_utf8_lossy(&name);
        let value = String::from_utf8_lossy(&value);

        headers.insert(name.to_string(), value.to_string());
    }

    let method = headers.get(":method").ok_or("Missing Method Header")?;
    let method = Method::from_str(method)?;
    let path = headers.get(":path").ok_or("Missing Method Header")?.clone();

    Ok(Request {
        headers,
        method,
        path,
        stream_id,
    })
}

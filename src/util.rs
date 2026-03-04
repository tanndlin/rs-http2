use std::{collections::HashMap, fs::read, str::FromStr};

use crate::{
    http2::{
        connection_state::ConnectionState,
        frames::{continuation_frame::ContinuationFrame, headers_frame::HeadersFrame},
        gc_buffer::GCBuffer,
    },
    request::{Method, Request},
    response::{Response, ResponseBuilder, StatusCode},
    types::ContentType,
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
        body: vec![],
    })
}

pub fn handle_request(request: &Request) -> Result<Response, String> {
    println!("Got request");
    // dbg!(&request);

    match request.method {
        Method::GET => handle_get(request),
        Method::HEAD => handle_head(request),
        _ => Ok(Response::method_not_allowed(request.stream_id)),
    }
}

fn handle_get(request: &Request) -> Result<Response, String> {
    let path = if &request.path == "/" {
        "/index.html"
    } else {
        &request.path
    };

    dbg!(&path);

    let file_extension = path.split(".").last().ok_or("No file extension found")?;
    let content_type = ContentType::from_extension(file_extension);
    if content_type == ContentType::Unknown {
        return Ok(Response::bad_request(request.stream_id));
    }

    let file_contents = read(format!("public{path}")).map_err(|_| "Unable to read file")?;
    Ok(ResponseBuilder::new()
        .status_code(StatusCode::Ok)
        .header("Content-Type".to_string(), content_type.into())
        .stream_id(request.stream_id)
        .body(file_contents)
        .build())
}

fn handle_head(request: &Request) -> Result<Response, String> {
    let path = if &request.path == "/" {
        "index.html"
    } else {
        &request.path
    };

    let file_extension = path.split(".").last().ok_or("No file extension found")?;
    let content_type = ContentType::from_extension(file_extension);
    if content_type == ContentType::Unknown {
        return Ok(Response::bad_request(request.stream_id));
    }

    let file_contents = read(format!("public{path}")).map_err(|_| "Unable to read file")?;
    Ok(ResponseBuilder::new()
        .status_code(StatusCode::Ok)
        .header("Content-Type".to_string(), content_type.into())
        .header(
            "Content-Length".to_string(),
            file_contents.len().to_string(),
        )
        .stream_id(request.stream_id)
        .body(file_contents)
        .build())
}

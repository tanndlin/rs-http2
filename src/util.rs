use std::fs::read;

use crate::{
    request::{Method, Request},
    response::{Response, ResponseBuilder, StatusCode},
    types::ContentType,
};

pub fn u32_from_3_bytes(buf: [u8; 3]) -> u32 {
    u32::from(buf[0]) << 16 | u32::from(buf[1]) << 8 | u32::from(buf[2])
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
    let file_extension = request
        .path
        .split('.')
        .next_back()
        .ok_or("No file extension found")?;
    let content_type = ContentType::from_extension(file_extension);
    if content_type == ContentType::Unknown {
        return Ok(Response::bad_request(request.stream_id));
    }

    match read(request.path.clone()) {
        Ok(file_contents) => Ok(ResponseBuilder::new()
            .status_code(StatusCode::Ok)
            .header("Content-Type".to_string(), content_type.into())
            .stream_id(request.stream_id)
            .body(file_contents)
            .build()),
        Err(_) => Ok(Response::not_found(request.stream_id)),
    }
}

fn handle_head(request: &Request) -> Result<Response, String> {
    let file_extension = request
        .path
        .split('.')
        .next_back()
        .ok_or("No file extension found")?;
    let content_type = ContentType::from_extension(file_extension);
    if content_type == ContentType::Unknown {
        return Ok(Response::bad_request(request.stream_id));
    }

    let file_contents = read(request.path.clone()).map_err(|_| "Unable to read file")?;
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

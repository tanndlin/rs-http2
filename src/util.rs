use std::{collections::HashMap, sync::Arc};

use crate::{
    request::{Method, Request},
    response::{Response, ResponseBuilder, StatusCode},
    types::ContentType,
};

pub fn u32_from_3_bytes(buf: [u8; 3]) -> u32 {
    u32::from(buf[0]) << 16 | u32::from(buf[1]) << 8 | u32::from(buf[2])
}

pub fn handle_request(
    request: &Request,
    cache: &Arc<HashMap<String, Vec<u8>>>,
) -> Result<Response, String> {
    match request.method {
        Method::GET => handle_get(request, cache),
        Method::HEAD => handle_head(request, cache),
        _ => Ok(Response::method_not_allowed(request.stream_id)),
    }
}

fn handle_get(
    request: &Request,
    cache: &Arc<HashMap<String, Vec<u8>>>,
) -> Result<Response, String> {
    let file_extension = request
        .path
        .split('.')
        .next_back()
        .ok_or("No file extension found")?;
    let content_type = ContentType::from_extension(file_extension);
    if content_type == ContentType::Unknown {
        return Ok(Response::bad_request(request.stream_id));
    }

    let slice = if let Some(range) = request.headers.get("range") {
        let range = range.strip_prefix("bytes=").ok_or("Invalid range header")?;
        let mut split = range.split('-');
        let start = split
            .next()
            .ok_or("Invalid range header")?
            .parse::<usize>()
            .map_err(|_| "Invalid range header")?;

        let end = if let Some(end) = split.next()
            && !end.is_empty()
        {
            Some(end.parse::<usize>().map_err(|_| "Invalid range header")?)
        } else {
            None
        };
        Some((start, end))
    } else {
        None
    };

    match cache.get(&request.path) {
        Some(bytes) => {
            let bytes = if let Some((start, end)) = slice {
                if start >= bytes.len() {
                    return Ok(Response::range_not_satisfiable(request.stream_id));
                }
                let end = end.unwrap_or(bytes.len());
                if end > bytes.len() {
                    return Ok(Response::range_not_satisfiable(request.stream_id));
                }
                bytes[start..end].to_vec()
            } else {
                bytes.clone()
            };

            let len = bytes.len();

            let mut builder = ResponseBuilder::new()
                .stream_id(request.stream_id)
                .header("content-type".to_string(), content_type.to_string())
                .header("content-length".to_string(), len.to_string())
                .body(bytes);

            if slice.is_some() {
                builder = builder.status_code(StatusCode::PartialContent).header(
                    "content-range".to_string(),
                    format!(
                        "bytes {}-{}/{}",
                        slice.unwrap().0,
                        slice.unwrap().1.unwrap_or(len - 1),
                        len
                    ),
                );
            } else {
                builder = builder.status_code(StatusCode::Ok);
            }

            Ok(builder.build())
        }
        None => Ok(Response::not_found(request.stream_id)),
    }
}

fn handle_head(
    request: &Request,
    cache: &Arc<HashMap<String, Vec<u8>>>,
) -> Result<Response, String> {
    let file_extension = request
        .path
        .split('.')
        .next_back()
        .ok_or("No file extension found")?;
    let content_type = ContentType::from_extension(file_extension);
    if content_type == ContentType::Unknown {
        return Ok(Response::bad_request(request.stream_id));
    }

    let slice = if let Some(range) = request.headers.get("range") {
        let range = range.strip_prefix("bytes=").ok_or("Invalid range header")?;
        let mut split = range.split('-');
        let start = split
            .next()
            .ok_or("Invalid range header")?
            .parse::<usize>()
            .map_err(|_| "Invalid range header")?;

        let end = if let Some(end) = split.next()
            && !end.is_empty()
        {
            Some(end.parse::<usize>().map_err(|_| "Invalid range header")?)
        } else {
            None
        };
        Some((start, end))
    } else {
        None
    };

    match cache.get(&request.path) {
        Some(bytes) => {
            let len = bytes.len();

            let mut builder = ResponseBuilder::new()
                .stream_id(request.stream_id)
                .header("content-type".to_string(), content_type.to_string())
                .header("content-length".to_string(), len.to_string());

            if slice.is_some() {
                builder = builder.status_code(StatusCode::PartialContent).header(
                    "content-range".to_string(),
                    format!(
                        "bytes {}-{}/{}",
                        slice.unwrap().0,
                        slice.unwrap().1.unwrap_or(len - 1),
                        len
                    ),
                );
            } else {
                builder = builder.status_code(StatusCode::Ok);
            }

            Ok(builder.build())
        }
        None => Ok(Response::not_found(request.stream_id)),
    }
}

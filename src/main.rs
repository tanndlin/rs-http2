use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
};

use crate::{
    read::cache_all_files,
    request::{Method, Request},
    response::{Response, ResponseBuilder},
    types::ContentType,
};

mod read;
mod request;
mod response;
mod types;

fn main() {
    // Log args
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        panic!("Expected 1 argument (serve folder)")
    }

    let listener = TcpListener::bind("0.0.0.0:8080").expect("Unable to bind to 0.0.0.0:8080");
    println!("Listening on: 0.0.0.0:8080");
    println!("Serving files from: {}", args[1]);

    let cache = Arc::new(match cache_all_files(&args[1].clone()) {
        Ok(c) => c,
        Err(e) => panic!("{e}"),
    });

    let total_files = cache.len();
    let total_bytes: usize = cache.values().map(|b| b.len()).sum();
    println!("Cached {} files.", total_files);
    println!("Cached {total_bytes} bytes.");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let serve_path = args[1].clone().replace("\\", "/");
                let cache_clone = cache.clone();
                thread::spawn(move || handle_client(stream, &serve_path, &cache_clone));
            }
            Err(e) => println!("Unable to get stream from client: {e}"),
        }
    }
}

fn handle_client(
    mut stream: TcpStream,
    serve_location: &str,
    cache: &Arc<HashMap<String, Vec<u8>>>,
) {
    let mut buffer = [0u8; 1024];
    loop {
        let _ = stream.read(&mut buffer);

        let (response, keep_alive) = match Request::from_bytes(buffer) {
            Ok(mut req) => {
                req.path = if req.path == "/" {
                    "/index.html".to_string()
                } else {
                    req.path
                };
                req.path = format!("{}{}", serve_location, req.path);

                let keep_alive = match req.get_header("Connection") {
                    Some(header) => header.value == "Keep-Alive",
                    None => false,
                };
                match handle_request(&req, cache) {
                    Ok(res) => (res, keep_alive),
                    Err(e) => {
                        println!("Encountered server error {e}");
                        (Response::internal_server_error(), false)
                    }
                }
            }
            Err(_) => (Response::bad_request(), false),
        };

        let _ = stream.write(&response.to_bytes());

        if !keep_alive {
            break;
        }
    }
}

fn handle_request(
    request: &Request,
    cache: &Arc<HashMap<String, Vec<u8>>>,
) -> Result<Response, String> {
    match request.method {
        Method::GET => handle_get(request, cache),
        Method::HEAD => handle_head(request, cache),
        _ => Ok(Response::method_not_allowed()),
    }
}

fn handle_get(
    request: &Request,
    cache: &Arc<HashMap<String, Vec<u8>>>,
) -> Result<Response, String> {
    let file_extension = &request
        .path
        .split(".")
        .last()
        .ok_or("No file extension found")?;
    let content_type = ContentType::from_extension(file_extension);
    if content_type == ContentType::Unknown {
        return Ok(Response::bad_request());
    }

    match cache.get(&request.path) {
        Some(contents) => Ok(ResponseBuilder::new()
            .status_code(response::StatusCode::Ok)
            .header("Content-Type".to_string(), content_type.into())
            .body(contents.clone())
            .build()),
        None => Ok(Response::not_found()),
    }
}

fn handle_head(
    request: &Request,
    cache: &Arc<HashMap<String, Vec<u8>>>,
) -> Result<Response, String> {
    let file_extension = &request
        .path
        .split(".")
        .last()
        .ok_or("No file extension found")?;
    let content_type = ContentType::from_extension(file_extension);
    if content_type == ContentType::Unknown {
        return Ok(Response::bad_request());
    }

    match cache.get(&request.path) {
        Some(metadata) => Ok(ResponseBuilder::new()
            .status_code(response::StatusCode::Ok)
            .header("Content-Type".to_string(), content_type.into())
            .header("Content-Length".to_string(), metadata.len().to_string())
            .build()),
        None => Ok(Response::not_found()),
    }
}

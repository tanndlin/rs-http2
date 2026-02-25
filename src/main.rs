use std::{
    fs::{metadata, read},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};

use crate::{
    request::{Method, Request},
    response::{Response, ResponseBuilder},
    types::ContentType,
};

mod request;
mod response;
mod types;

fn main() {
    // Log args
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        panic!("Expected 1 argument (serve folder)")
    }

    let listener = TcpListener::bind("127.0.0.1:8080").expect("Unable to bind to 127.0.0.1:8080");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let serve_location = args[1].clone();
                thread::spawn(move || handle_client(stream, serve_location));
            }
            Err(e) => println!("Unable to get stream from client: {e}"),
        }
    }
}

fn handle_client(mut stream: TcpStream, serve_location: String) {
    let mut buffer = [0u8; 1024];
    loop {
        let _ = stream.read(&mut buffer);

        let (response, keep_alive) = match Request::from_bytes(buffer) {
            Ok(req) => {
                let keep_alive = match req.get_header("Connection") {
                    Some(header) => header.value == "Keep-Alive",
                    None => false,
                };
                match handle_request(&req, &serve_location) {
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

fn handle_request(request: &Request, serve_location: &str) -> Result<Response, String> {
    match request.method {
        Method::GET => handle_get(request, serve_location),
        Method::HEAD => handle_head(request, serve_location),
        _ => Ok(Response::method_not_allowed()),
    }
}

fn handle_get(request: &Request, serve_location: &str) -> Result<Response, String> {
    let path = if request.path == "/" {
        "index.html"
    } else {
        request.path.trim_start_matches("/")
    };

    let full_path = format!("{}/{}", serve_location, path);
    let file_extension = full_path
        .split(".")
        .last()
        .ok_or("No file extension found")?;
    let content_type = ContentType::from_extension(file_extension);
    if content_type == ContentType::Unknown {
        return Ok(Response::bad_request());
    }

    match read(full_path) {
        Ok(contents) => Ok(ResponseBuilder::new()
            .status_code(response::StatusCode::Ok)
            .header("Content-Type".to_string(), content_type.into())
            .body(contents)
            .build()),
        Err(_) => Ok(Response::not_found()),
    }
}

fn handle_head(request: &Request, serve_location: &str) -> Result<Response, String> {
    let path = if request.path == "/" {
        "index.html"
    } else {
        request.path.trim_start_matches("/")
    };

    let full_path = format!("{}/{}", serve_location, path);
    let file_extension = full_path
        .split(".")
        .last()
        .ok_or("No file extension found")?;
    let content_type = ContentType::from_extension(file_extension);
    if content_type == ContentType::Unknown {
        return Ok(Response::bad_request());
    }

    match metadata(full_path) {
        Ok(metadata) => Ok(ResponseBuilder::new()
            .status_code(response::StatusCode::Ok)
            .header("Content-Type".to_string(), content_type.into())
            .header("Content-Length".to_string(), metadata.len().to_string())
            .build()),
        Err(_) => Ok(Response::not_found()),
    }
}

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread::available_parallelism,
};

use crate::{
    read::cache_all_files,
    request::{Method, Request},
    response::{Response, ResponseBuilder},
    settings_frame::SettingsFrame,
    types::ContentType,
};

use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream};
use threadpool::ThreadPool;

mod read;
mod request;
mod response;
mod settings_frame;
mod types;

fn main() {
    // Log args
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        panic!("Expected 1 argument (serve folder)")
    }

    // Build TLS acceptor
    let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    builder
        .set_private_key_file("localhost+1-key.pem", SslFiletype::PEM)
        .unwrap();
    builder
        .set_certificate_chain_file("localhost+1.pem")
        .unwrap();

    // Enable HTTP/2 via ALPN
    builder.set_alpn_select_callback(|_, client_protocols| {
        openssl::ssl::select_next_proto(b"\x02h2\x08http/1.1", client_protocols)
            .ok_or(openssl::ssl::AlpnError::NOACK)
    });

    let acceptor = Arc::new(builder.build());

    let listener = TcpListener::bind("0.0.0.0:443").expect("Unable to bind to 0.0.0.0:443");
    println!("Listening on: 0.0.0.0:443");
    println!("Serving files from: {}", args[1]);

    let cache = Arc::new(match cache_all_files(&args[1].clone()) {
        Ok(c) => c,
        Err(e) => panic!("{e}"),
    });

    let total_files = cache.len();
    let total_bytes: usize = cache.values().map(|b| b.len()).sum();
    println!("Cached {total_bytes} bytes across {total_files} files.");

    let num_cores = available_parallelism().unwrap().get();
    let pool = ThreadPool::new(num_cores);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let acceptor = acceptor.clone();
                let ssl_stream = acceptor.accept(stream).unwrap();

                let serve_path = args[1].clone().replace("\\", "/");
                let cache_clone = cache.clone();
                pool.execute(move || handle_client(ssl_stream, &serve_path, &cache_clone));
            }
            Err(e) => println!("Unable to get stream from client: {e}"),
        }
    }
}

fn handle_client(
    mut stream: SslStream<TcpStream>,
    serve_location: &str,
    cache: &Arc<HashMap<String, Vec<u8>>>,
) {
    let mut buffer = [0u8; 1024];

    // Should start with the HTTP/2 Connection Preface
    let read = stream.read(&mut buffer).unwrap();
    if buffer[..24] != b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"[..] {
        return;
    }

    let settings = SettingsFrame::from_bytes(&buffer[24..read]).unwrap();
    dbg!(&settings);

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break, // Client closed connection
            Ok(_) => (),
            Err(_) => break,
        }

        dbg!(String::from_utf8_lossy(&buffer));

        let (response, keep_alive) = match Request::from_bytes(buffer) {
            Ok(mut req) => {
                req.path = if req.path == "/" {
                    "/index.html".to_string()
                } else {
                    req.path
                };
                req.path = format!("{}{}", serve_location, req.path);

                dbg!(&req.headers);

                let keep_alive = match req.headers.get("Connection") {
                    Some(value) => value == "Close",
                    None => true,
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

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    str::FromStr,
    sync::Arc,
    thread::available_parallelism,
};

use crate::{
    http2::{
        connection_state::ConnectionState,
        frames::{
            continuation_frame::ContinuationFrame,
            data_frame::DataFrame,
            frame::{self, Frame, FrameHeader, FrameType},
            headers_frame::HeadersFrame,
            ping_frame::PingFrame,
            settings_frame::{SettingsFrame, SettingsFrameFlags},
        },
        gc_buffer::GCBuffer,
    },
    read::cache_all_files,
    request::{Method, Request},
    response::{Response, ResponseBuilder},
    types::ContentType,
    util::u32_from_3_bytes,
};

use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream};
use threadpool::ThreadPool;

mod http2;
mod read;
mod request;
mod response;
mod types;
mod util;

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

                let cache_clone = cache.clone();
                pool.execute(move || handle_client(ssl_stream, &cache_clone));
            }
            Err(e) => println!("Unable to get stream from client: {e}"),
        }
    }
}

fn handle_client(mut stream: SslStream<TcpStream>, cache: &Arc<HashMap<String, Vec<u8>>>) {
    let mut state = ConnectionState::new();

    // Should start with the HTTP/2 Connection Preface
    let mut preface = [0; 24];
    let _ = stream.read_exact(&mut preface).unwrap();
    if preface != b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"[..] {
        return;
    }

    // TODO: Make sure first frame is settings

    let mut buffer = GCBuffer::new();
    loop {
        match buffer.read_from_stream(&mut stream) {
            Ok(0) => {
                dbg!("Client closed connection");
                break;
            } // Client closed connection
            Ok(read) => {
                dbg!(read);
                read
            }
            Err(e) => {
                dbg!("Error reading from stream", e);
                break;
            }
        };

        let length = u32_from_3_bytes(buffer.peek::<3>());
        dbg!(&length);
        let full_frame_length = (length + 9) as usize;
        if buffer.len() < full_frame_length {
            continue;
        }

        println!("Parsing frame of length {full_frame_length}");

        let req = match frame::Frame::try_from(&buffer.read_n_bytes(full_frame_length)[..]) {
            Err(e) => {
                dbg!(&e);
                break;
            }
            Ok(f) => {
                dbg!(&f);
                match f {
                    Frame::Data(data_frame) => {
                        handle_data_frame(data_frame);
                        None
                    }
                    Frame::Headers(headers_frame) => {
                        Some(handle_headers_frame(&mut buffer, &headers_frame, &mut state).unwrap())
                    }
                    Frame::Settings(settings_frame) => {
                        if settings_frame.header.flags.ack {
                            continue;
                        }

                        let stream_id = settings_frame.header.stream_id;

                        // Send my settings
                        // TODO: Make a builder or something for SettingsFrame instantiation
                        let header = FrameHeader::<SettingsFrameFlags> {
                            length: 0,
                            frame_type: FrameType::Settings,
                            flags: SettingsFrameFlags { ack: false },
                            stream_id,
                        };
                        let frame_bytes: Vec<u8> = header.into();
                        let _ = stream.write(&frame_bytes);

                        // Send ack
                        let ack = SettingsFrame::new_ack(0);
                        let bytes: Vec<u8> = ack.into();
                        let _ = stream.write(&bytes);
                        None
                    }
                    Frame::Ping(ping_frame) => {
                        if !ping_frame.header.flags.ack {
                            let ack = PingFrame::ack();
                            let bytes: Vec<u8> = ack.into();
                            let _ = stream.write(&bytes);
                        }

                        None
                    }
                    Frame::Priority(priority_frame) => handle_priority_frame(priority_frame),
                    Frame::RstStream(rst_frame) => handle_rst_frame(rst_frame),
                }
            }
        };

        if let Some(req) = req {
            match handle_request(&req, cache) {
                Err(e) => {
                    dbg!(&e);
                }
                Ok(res) => match send_response(&mut stream, &res, &mut state) {
                    Ok(_) => (),
                    Err(e) => {
                        dbg!(&e);
                    }
                },
            }
        }
    }

    println!("Outside read loop");
}

fn handle_rst_frame(rst_frame: http2::frames::rst_frame::RstFrame) -> Option<Request> {
    println!("recv rst frame");
    None
}

fn handle_priority_frame(
    priority_frame: http2::frames::priority_frame::PriorityFrame,
) -> Option<Request> {
    println!("recv prio frame");
    None
}

fn handle_data_frame(data_frame: DataFrame) {
    println!("Received data frame");
}

fn handle_headers_frame(
    buffer: &mut GCBuffer,
    headers_frame: &HeadersFrame,
    state: &mut ConnectionState,
) -> Result<Request, String> {
    dbg!("Handling received headers frame");

    let mut compressed_headers = headers_frame.header_block_fragment.clone();
    if !headers_frame.header.flags.end_headers {
        loop {
            let length = u32_from_3_bytes(buffer.peek::<3>()) as usize;
            let buffer = buffer.read_n_bytes(length + 9);
            let next_frame = ContinuationFrame::try_from(&buffer[..]).unwrap();
            compressed_headers.extend_from_slice(&next_frame.header_block_fragment);

            if next_frame.header.flags.end_headers {
                break;
            }
        }
    }

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

    let stream_id = headers_frame.header.stream_id;

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

fn handle_request(
    request: &Request,
    cache: &Arc<HashMap<String, Vec<u8>>>,
) -> Result<Response, String> {
    println!("Got request");
    dbg!(&request);

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
    let path = if &request.path == "/" {
        "/index.html"
    } else {
        &request.path
    };

    dbg!(&path);

    let file_extension = path.split(".").last().ok_or("No file extension found")?;
    let content_type = ContentType::from_extension(file_extension);
    if content_type == ContentType::Unknown {
        return Ok(Response::bad_request());
    }

    match cache.get(path) {
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
    let path = if &request.path == "/" {
        "index.html"
    } else {
        &request.path
    };

    let file_extension = path.split(".").last().ok_or("No file extension found")?;
    let content_type = ContentType::from_extension(file_extension);
    if content_type == ContentType::Unknown {
        return Ok(Response::bad_request());
    }

    match cache.get(path) {
        Some(metadata) => Ok(ResponseBuilder::new()
            .status_code(response::StatusCode::Ok)
            .header("Content-Type".to_string(), content_type.into())
            .header("Content-Length".to_string(), metadata.len().to_string())
            .build()),
        None => Ok(Response::not_found()),
    }
}

fn send_response(
    stream: &mut SslStream<TcpStream>,
    res: &Response,
    state: &mut ConnectionState,
) -> Result<(), String> {
    let headers_frame = HeadersFrame::from((res, state));
    let bytes: Vec<u8> = headers_frame.into();
    let _ = stream.write(&bytes);

    let data_frame = DataFrame::from(res);
    let bytes: Vec<u8> = data_frame.into();
    let _ = stream.write(&bytes);
    Ok(())
}

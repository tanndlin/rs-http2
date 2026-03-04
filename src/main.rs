use std::{
    collections::HashMap,
    fs::read,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread::available_parallelism,
};

use crate::{
    http2::{
        connection_state::ConnectionState,
        error::HTTP2Error,
        frames::{
            frame::{self, Frame, FrameHeader, FrameType},
            go_away_frame::GoAwayFrame,
            ping_frame::PingFrame,
            rst_frame::RstFrame,
            settings_frame::{SettingsFrame, SettingsFrameFlags},
        },
        gc_buffer::GCBuffer,
        stream::http_stream::HTTP2Stream,
    },
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
    // println!("Serving files from: {}", args[1]);

    let num_cores = available_parallelism().unwrap().get();
    let pool = ThreadPool::new(num_cores);

    for tcp_stream in listener.incoming() {
        match tcp_stream {
            Ok(tcp_stream) => {
                let acceptor = acceptor.clone();
                let peer_id = tcp_stream.peer_addr().unwrap();
                dbg!(peer_id);
                let ssl_stream = acceptor.accept(tcp_stream).unwrap();

                pool.execute(move || handle_client(ssl_stream));
            }
            Err(e) => println!("Unable to get stream from client: {e}"),
        }
    }
}

fn handle_client(mut tcp_stream: SslStream<TcpStream>) {
    let mut state = ConnectionState::new();
    let mut streams: HashMap<u32, HTTP2Stream> = HashMap::new();

    // Should start with the HTTP/2 Connection Preface
    let mut preface = [0; 24];
    tcp_stream.read_exact(&mut preface).unwrap();
    if preface != b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"[..] {
        println!("Didn't recv preface, dropping client");
        return;
    }

    // TODO: Make sure first frame is settings

    let mut buffer = GCBuffer::new();
    loop {
        dbg!("Reading");
        match buffer.read_from_stream(&mut tcp_stream) {
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

        let result = match frame::Frame::try_from(&buffer.read_n_bytes(full_frame_length)[..]) {
            Err(e) => {
                dbg!(&e);
                break;
            }
            Ok(f) => {
                dbg!(&f);
                let stream_id = f.get_stream_id();

                match f {
                    Frame::Settings(settings_frame) => {
                        handle_settings_frame(&mut tcp_stream, settings_frame)
                    }
                    Frame::Ping(ping_frame) => handle_ping_frame(&mut tcp_stream, ping_frame),
                    _ => {
                        // TODO: See if there is a way to do state management without push and pop
                        let stream = streams
                            .remove(&stream_id)
                            .or_else(|| Some(HTTP2Stream::new(stream_id)))
                            .unwrap();

                        match stream.handle_frame(f, &mut state) {
                            Ok((stream_state, bytes)) => {
                                println!("writing Ok to {stream_id}");
                                streams.insert(stream_id, stream_state);
                                Ok(bytes)
                            }
                            Err((stream_state, e)) => {
                                println!("writing Err to {stream_id}");
                                streams.insert(stream_id, stream_state);
                                Err(e)
                            }
                        }
                    }
                }
            }
        };

        match result {
            Ok(bytes) => {
                let _ = tcp_stream.write(&bytes);
            }
            Err(e) => match e {
                HTTP2Error::Connection(e) => {
                    let go_away = GoAwayFrame::from(e);
                    let bytes: Vec<u8> = go_away.into();
                    let _ = tcp_stream.write(&bytes);
                }
                HTTP2Error::Stream(e) => {
                    let rst = RstFrame::from(e);
                    let bytes: Vec<u8> = rst.into();
                    let _ = tcp_stream.write(&bytes);
                }
            },
        }
    }

    println!("Outside read loop");
}

fn handle_settings_frame(
    tcp_stream: &mut SslStream<TcpStream>,
    settings_frame: SettingsFrame,
) -> Result<Vec<u8>, HTTP2Error> {
    if settings_frame.header.flags.ack {
        return Ok(vec![]);
    }

    let stream_id = settings_frame.header.stream_id;
    let header = FrameHeader::<SettingsFrameFlags> {
        length: 0,
        frame_type: FrameType::Settings,
        flags: SettingsFrameFlags { ack: false },
        stream_id,
    };
    let frame_bytes: Vec<u8> = header.into();
    let _ = tcp_stream.write(&frame_bytes);
    let ack = SettingsFrame::new_ack(0);
    let bytes: Vec<u8> = ack.into();
    let _ = tcp_stream.write(&bytes);

    Ok(vec![])
}

fn handle_ping_frame(
    tcp_stream: &mut SslStream<TcpStream>,
    ping_frame: PingFrame,
) -> Result<Vec<u8>, HTTP2Error> {
    if !ping_frame.header.flags.ack {
        let ack = PingFrame::ack();
        let bytes: Vec<u8> = ack.into();
        let _ = tcp_stream.write(&bytes);
    }
    Ok(vec![])
}

fn handle_request(request: &Request) -> Result<Response, String> {
    println!("Got request");
    dbg!(&request);

    match request.method {
        Method::GET => handle_get(request),
        Method::HEAD => handle_head(request),
        _ => Ok(Response::method_not_allowed()),
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
        return Ok(Response::bad_request());
    }

    let file_contents = read(format!("public{path}")).map_err(|_| "Unable to read file")?;
    Ok(ResponseBuilder::new()
        .status_code(response::StatusCode::Ok)
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
        return Ok(Response::bad_request());
    }

    let file_contents = read(format!("public{path}")).map_err(|_| "Unable to read file")?;
    Ok(ResponseBuilder::new()
        .status_code(response::StatusCode::Ok)
        .header("Content-Type".to_string(), content_type.into())
        .header(
            "Content-Length".to_string(),
            file_contents.len().to_string(),
        )
        .stream_id(request.stream_id)
        .body(file_contents)
        .build())
}

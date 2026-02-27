use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread::available_parallelism,
};

use crate::{
    http2::frames::{
        data_frame::DataFrame,
        frame::{self, Frame, FrameHeader, FrameType},
        frame_trait::Frame as _,
        headers_frame::{self, HeadersFrame},
        settings_frame::SettingsFrame,
    },
    read::cache_all_files,
    request::{Method, Request},
    response::{Response, ResponseBuilder},
    types::ContentType,
};

use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream};
use threadpool::ThreadPool;

mod http2;
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
    // Should start with the HTTP/2 Connection Preface

    let mut preface = [0; 24];
    let _ = stream.read_exact(&mut preface).unwrap();
    if preface != b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"[..] {
        return;
    }

    // TODO: Make sure first frame is settings

    // Send my settings
    // TODO: Make a builder or something for SettingsFrame instantiation
    let header = FrameHeader {
        length: 0,
        frame_type: FrameType::Settings,
        flags: 0,
        stream_identifier: 0,
    };
    let frame_bytes: Vec<u8> = header.into();
    let _ = stream.write(&frame_bytes);

    let mut buffer = [0u8; 1024];
    loop {
        let read = match stream.read(&mut buffer) {
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

        match frame::Frame::try_from(&buffer[..read]) {
            Err(e) => {
                dbg!(&e);
                break;
            }
            Ok(f) => {
                dbg!(&f);
                match f {
                    Frame::DataFrame(data_frame) => handle_data_frame(data_frame),
                    Frame::HeadersFrame(headers_frame) => {
                        handle_headers_frame(&mut stream, &headers_frame).unwrap();
                    }
                    Frame::SettingsFrame(settings_frame) => {
                        if settings_frame.header.flags.ack {
                            continue;
                        }

                        // Send ack
                        // TODO: Send correct stream ident
                        let ack = SettingsFrame::new_ack(0);
                        let bytes: Vec<u8> = ack.into();
                        let _ = stream.write(&bytes);
                    }
                }
            }
        }
    }

    println!("Outside read loop");
}

fn handle_data_frame(data_frame: DataFrame) {
    println!("Received data frame");
}

fn handle_headers_frame(
    stream: &mut SslStream<TcpStream>,
    headers_frame: &HeadersFrame,
) -> Result<(), String> {
    // match headers_frame.header.flags.end_headers {
    //     true => todo!(),
    //     false => todo!(),
    // }

    dbg!("Handling received headers frame");

    let mut compressed_headers = headers_frame.header_block_fragment.clone();
    if !headers_frame.header.flags.end_headers {
        // TODO: This length reconstruction is awful
        loop {
            let mut length_buffer = [0u8; 3];
            let to_read = stream.read_exact(&mut length_buffer);
            let length = {
                let mut padded = [0; 4];
                padded.copy_from_slice(&length_buffer);
                u32::from_be_bytes(padded) as usize
            };

            let mut buffer = vec![0u8; length + 6];
            let _ = stream.read_exact(&mut buffer);
            // put the 3 length bytes back
            let mut buffer_with_length = vec![];
            buffer_with_length.extend_from_slice(&length_buffer);
            buffer_with_length.extend_from_slice(&buffer);

            let next_frame = HeadersFrame::try_from(&buffer[..]).unwrap();
            compressed_headers.extend_from_slice(&next_frame.header_block_fragment);

            if next_frame.header.flags.end_headers {
                break;
            }
        }
    }

    dbg!("Decoding");
    let mut decoder = hpack::Decoder::new();
    let decoded_headers = decoder
        .decode(&compressed_headers)
        .map_err(|e| format!("Error decoding compressed headers: {:?}", e))?;

    dbg!(&decoded_headers);

    for (name, value) in decoded_headers {
        let name = String::from_utf8_lossy(&name);
        let value = String::from_utf8_lossy(&value);
        dbg!(&name, &value);
    }

    let stream_ident = headers_frame.header.stream_identifier;

    Ok(())
}

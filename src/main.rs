use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread::available_parallelism,
};

use crate::{
    http2::{
        connection_state::ConnectionState,
        error::{HTTP2Error, HTTP2ErrorCode},
        frames::{
            frame::{self, Frame, FrameHeader, FrameType},
            go_away_frame::GoAwayFrame,
            ping_frame::PingFrame,
            rst_frame::RstFrame,
            settings_frame::{SettingsFrame, SettingsFrameBuilder, SettingsFrameFlags},
        },
        gc_buffer::GCBuffer,
        stream::http_stream::HTTP2Stream,
    },
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
                    Frame::Settings(settings_frame) => handle_settings_frame(settings_frame),
                    Frame::Ping(ping_frame) => handle_ping_frame(&mut tcp_stream, ping_frame),
                    _ => {
                        // TODO: See if there is a way to do state management without push and pop
                        let stream = match streams.remove(&stream_id) {
                            Some(s) => s,
                            None => {
                                if stream_id.is_multiple_of(2)
                                    || stream_id < streams.keys().copied().max().unwrap_or(0)
                                {
                                    let go_away = GoAwayFrame::from(HTTP2ErrorCode::ProtocolError);
                                    let bytes: Vec<u8> = go_away.into();
                                    let _ = tcp_stream.write(&bytes);
                                    return;
                                } else {
                                    HTTP2Stream::new(stream_id)
                                }
                            }
                        };

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

fn handle_settings_frame(settings_frame: SettingsFrame) -> Result<Vec<u8>, HTTP2Error> {
    if settings_frame.header.stream_id != 0 {
        return Err(HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError));
    }

    if settings_frame.header.flags.ack {
        return Ok(vec![]);
    }

    let my_settings = SettingsFrameBuilder::new()
        .enable_push(false)
        .header_table_size(4096)
        // .max_concurrent_streams(max) // unlimited
        .initial_window_size(65535)
        .max_frame_size(16384)
        // .max_header_list_size(size) // unlimited
        .build();

    dbg!(&my_settings);
    let mut frame_bytes: Vec<u8> = my_settings.into();

    let ack = SettingsFrame::new_ack(0);
    let ack_bytes: Vec<u8> = ack.into();

    frame_bytes.extend_from_slice(&ack_bytes);
    Ok(frame_bytes)
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

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread::available_parallelism,
};

use crate::{
    encode_to::EncodeTo,
    http2::{
        connection_state::ConnectionState,
        error::{HTTP2Error, HTTP2ErrorCode},
        frames::{
            frame::{self, Frame},
            go_away_frame::GoAwayFrame,
            ping_frame::PingFrame,
            rst_frame::RstFrame,
            settings_frame::{SettingsFrame, SettingsFrameBuilder},
        },
        gc_buffer::GCBuffer,
        stream::http_stream::HTTP2Stream,
    },
    util::u32_from_3_bytes,
};

use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream};
use threadpool::ThreadPool;

mod encode_to;
mod http2;
mod read;
mod request;
mod response;
mod types;
mod util;

const MAX_FRAME_SIZE: u32 = 1 << 14; // 16384

fn main() {
    // Log args
    let args: Vec<String> = std::env::args().collect();
    assert!(args.len() == 2, "Expected 1 argument (serve folder)");

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

macro_rules! read_or_return {
    ($buffer:expr, $stream:expr) => {
        match $buffer.read_from_stream($stream) {
            Ok(0) => {
                println!("Client closed connection");
                return;
            }
            Ok(_) => continue,
            Err(e) => {
                println!("Error reading from stream: {e}");
                return;
            }
        }
    };
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

    let mut buffer = GCBuffer::new();
    loop {
        // Check if there is a frame in the buffer, otherwise read and continue
        let full_frame_length = match buffer.peek::<3>() {
            Some(len_buf) => (u32_from_3_bytes(*len_buf) + 9) as usize,
            None => read_or_return!(buffer, &mut tcp_stream),
        };

        if buffer.len() < full_frame_length {
            read_or_return!(buffer, &mut tcp_stream);
        }

        println!("Parsing frame of length {full_frame_length}");

        let result = match frame::Frame::try_from(&buffer.read_n_bytes(full_frame_length)[..]) {
            Err(e) => {
                dbg!(&e);
                break;
            }
            Ok(f) => {
                // dbg!(&f);
                let stream_id = f.get_stream_id();

                match f {
                    Frame::Settings(settings_frame) => handle_settings_frame(&settings_frame),
                    Frame::Ping(ping_frame) => Ok(handle_ping_frame(&mut tcp_stream, &ping_frame)),
                    _ => {
                        // TODO: See if there is a way to do state management without push and pop
                        let stream = if let Some(s) = streams.remove(&stream_id) {
                            s
                        } else {
                            if stream_id.is_multiple_of(2)
                                || stream_id < streams.keys().copied().max().unwrap_or(0)
                            {
                                let go_away = GoAwayFrame::from(HTTP2ErrorCode::ProtocolError);
                                let _ = tcp_stream.write(&go_away.to_bytes());
                                return;
                            }

                            HTTP2Stream::new(stream_id)
                        };

                        // Check if the size is greater than max frame size, if so send a GOAWAY and close the connection
                        if full_frame_length - 9 > MAX_FRAME_SIZE as usize {
                            println!(
                                "Received frame larger than max frame size, sending GOAWAY and closing connection"
                            );
                            let go_away = GoAwayFrame::from(HTTP2ErrorCode::FrameSizeError);
                            let _ = tcp_stream.write(&go_away.to_bytes());
                            return;
                        }

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
                    let _ = tcp_stream.write(&go_away.to_bytes());
                }
                HTTP2Error::Stream(e) => {
                    let rst = RstFrame::from(e);
                    let _ = tcp_stream.write(&rst.to_bytes());
                }
            },
        }
    }

    println!("Outside read loop");
}

fn handle_settings_frame(settings_frame: &SettingsFrame) -> Result<Vec<u8>, HTTP2Error> {
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
        .max_frame_size(MAX_FRAME_SIZE)
        // .max_header_list_size(size) // unlimited
        .build();

    dbg!(&my_settings);

    let mut ret = my_settings.to_bytes();
    SettingsFrame::new_ack(0).encode_to(&mut ret);
    Ok(ret)
}

fn handle_ping_frame(
    tcp_stream: &mut SslStream<TcpStream>,
    ping_frame: &PingFrame,
) -> std::vec::Vec<u8> {
    if !ping_frame.header.flags.ack {
        let ack = PingFrame::ack();
        let _ = tcp_stream.write(&ack.to_bytes());
    }

    vec![]
}

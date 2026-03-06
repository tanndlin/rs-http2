use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    ops::Div,
    path::PathBuf,
    sync::Arc,
    thread::available_parallelism,
};

use crate::{
    encode_to::EncodeTo,
    http2::{
        connection_state::ConnectionState,
        error::{HTTP2Error, HTTP2ErrorCode, StreamError},
        frames::{
            data_frame::{DataFrame, DataFrameFlags},
            frame::{Frame, FrameHeader, FrameType},
            go_away_frame::GoAwayFrame,
            ping_frame::PingFrame,
            rst_frame::RstFrame,
            settings_frame::{SettingsFrame, SettingsFrameBuilder},
        },
        gc_buffer::GCBuffer,
        stream::{http_stream::HTTP2Stream, stream_closed::HTTP2StreamClosed},
    },
    read::cache_all_files,
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

fn main() {
    // Log args
    let args: Vec<String> = std::env::args().collect();
    assert!(args.len() == 2, "Expected 1 argument (serve folder)");
    let serve_location = PathBuf::from(&args[1]);
    assert!(
        serve_location.is_dir(),
        "Serve location must be a directory, got {}",
        args[1]
    );
    println!("Serving files from: {}", serve_location.display());

    let cache = Arc::new(cache_all_files(serve_location.to_str().unwrap()).unwrap());
    println!("Cached {} files", cache.len());

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

    let num_cores = available_parallelism().unwrap().get();
    let pool = ThreadPool::new(num_cores);

    for tcp_stream in listener.incoming() {
        match tcp_stream {
            Ok(tcp_stream) => {
                let acceptor = acceptor.clone();
                let ssl_stream = acceptor.accept(tcp_stream).unwrap();
                let serve_location = serve_location.clone();
                let cache = cache.clone();

                pool.execute(move || handle_client(ssl_stream, serve_location, cache));
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
                break;
            }
            Ok(_) => continue,
            Err(e) => {
                println!("Error reading from stream: {e}");
                break;
            }
        }
    };
}

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_truncation)]
fn flush_outbound_frames(
    tcp_stream: &mut SslStream<TcpStream>,
    state: &mut ConnectionState<'_>,
    outbound: &mut VecDeque<Frame>,
) -> std::io::Result<()> {
    while let Some(frame) = outbound.pop_front() {
        match frame {
            Frame::Data(mut data_frame) => {
                let stream_window = *state
                    .stream_window_sizes
                    .entry(data_frame.header.stream_id)
                    .or_insert(state.settings.window_size);
                let available_window = (state.window_size)
                    .min(stream_window)
                    .min(state.settings.max_frame_size as i32);

                if available_window <= 0 {
                    outbound.push_front(Frame::Data(data_frame));
                    break;
                }

                let chunk_size = (data_frame.data.len()).min(available_window as usize);
                let remaining = data_frame.data.split_off(chunk_size);
                let send_end_stream = remaining.is_empty() && data_frame.header.flags.end_stream;

                let df = DataFrame::new(
                    data_frame.header.stream_id,
                    data_frame.data,
                    send_end_stream,
                );

                if send_end_stream {
                    let idx = data_frame.header.stream_id.div(2) as usize;
                    let stream = &state.streams[idx];
                    state.streams[idx] = stream.server_sent_es();
                }

                tcp_stream.write_all(&df.to_bytes())?;
                state.sent_data(data_frame.header.stream_id, chunk_size as i32);

                if !remaining.is_empty() {
                    outbound.push_front(Frame::Data(DataFrame {
                        header: FrameHeader::<DataFrameFlags> {
                            length: remaining.len() as u32,
                            frame_type: FrameType::Data,
                            flags: DataFrameFlags {
                                padding: false,
                                end_stream: data_frame.header.flags.end_stream,
                            },
                            stream_id: data_frame.header.stream_id,
                        },
                        data: remaining,
                        pad_length: 0,
                    }));
                }
            }
            frame => {
                tcp_stream.write_all(&frame.to_bytes())?;
            }
        }
    }

    Ok(())
}

fn handle_client(
    mut tcp_stream: SslStream<TcpStream>,
    serve_location: PathBuf,
    cache: Arc<HashMap<String, Vec<u8>>>,
) {
    let mut state = ConnectionState::new(serve_location, cache);

    // Should start with the HTTP/2 Connection Preface
    let mut preface = [0; 24];
    match tcp_stream.read_exact(&mut preface) {
        Ok(()) => (),
        Err(e) => {
            println!("Error reading preface from stream: {e}");
            return;
        }
    }
    if preface != b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"[..] {
        println!("Didn't recv preface, dropping client");
        return;
    }

    let mut buffer = GCBuffer::new();
    let mut outbound = VecDeque::new();
    loop {
        if let Err(e) = flush_outbound_frames(&mut tcp_stream, &mut state, &mut outbound) {
            println!("Error writing frame to stream: {e}");
            break;
        }

        // Check if there is a frame in the buffer, otherwise read and continue
        let full_frame_length = match buffer.peek::<3>() {
            Some(len_buf) => (u32_from_3_bytes(*len_buf) + 9) as usize,
            None => read_or_return!(buffer, &mut tcp_stream),
        };

        if buffer.len() < full_frame_length {
            read_or_return!(buffer, &mut tcp_stream);
        }

        let result = match Frame::try_from(&buffer.read_n_bytes(full_frame_length)[..]) {
            Ok(frame) => handle_frame(&mut state, full_frame_length, frame),
            Err(e) => {
                // If we were waiting on conitunation frames, then this is a connection error, otherwise pass along the error
                if state.waiting_for_continuation.is_some() {
                    Err(HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError))
                } else {
                    Err(e)
                }
            }
        };

        match result {
            Ok(frames) => {
                for frame in frames {
                    match frame {
                        Frame::Data(df) => outbound.push_back(Frame::Data(df)),
                        _ => {
                            tcp_stream.write_all(&frame.to_bytes()).unwrap();
                        }
                    }
                }
            }
            Err(e) => match e {
                HTTP2Error::Connection(HTTP2ErrorCode::NoError) => (),
                HTTP2Error::Connection(e) => {
                    let go_away = GoAwayFrame::from(e);
                    if let Err(write_error) = tcp_stream.write_all(&go_away.to_bytes()) {
                        println!("Error writing GOAWAY frame to stream: {write_error}");
                    }
                    break;
                }
                HTTP2Error::Stream(e) => {
                    let rst = RstFrame::from(e);
                    if let Err(write_error) = tcp_stream.write_all(&rst.to_bytes()) {
                        println!("Error writing RST_STREAM frame to stream: {write_error}");
                        break;
                    }
                }
            },
        }
    }
}

fn handle_frame(
    state: &mut ConnectionState<'_>,
    full_frame_length: usize,
    frame: Frame,
) -> Result<Vec<Frame>, HTTP2Error> {
    // dbg!(&frame);
    let stream_id = frame.get_stream_id();

    match frame {
        Frame::PushPromise(_) => Err(HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError)),
        Frame::Settings(settings_frame) => handle_settings_frame(&settings_frame, state),
        Frame::Ping(ping_frame) => handle_ping_frame(ping_frame),
        _ => {
            // Determine if any stream is waiting for a continuation frame and, if so, which one.
            let waiting_for_continuation_stream_id = state.waiting_for_continuation;

            // If we're waiting for a continuation frame on a specific stream, then:
            // - Only CONTINUATION frames
            // - On that same stream_id
            // are allowed. Otherwise, send a GOAWAY and close the connection.
            if let Some(waiting_id) = waiting_for_continuation_stream_id
                && (waiting_id != stream_id || !matches!(frame, Frame::Continuation(_)))
            {
                println!(
                    "Received invalid frame while waiting for continuation, sending GOAWAY and closing connection"
                );
                return Err(HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError));
            }

            if let Frame::WindowUpdate(window_update) = &frame {
                if window_update.window_size_increment == 0 {
                    match window_update.header.stream_id {
                        0 => {
                            println!(
                                "Received invalid window update with stream id 0, sending GOAWAY and closing connection"
                            );
                            return Err(HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError));
                        }
                        stream_id => {
                            return Err(HTTP2Error::Stream(StreamError {
                                stream_id,
                                error_code: HTTP2ErrorCode::ProtocolError,
                            }));
                        }
                    }
                }

                if window_update.header.stream_id == 0 {
                    state.update_window(window_update)?;
                    return Ok(vec![]);
                }
            }

            let idx = stream_id.div(2) as usize;
            let stream = if state.streams.len() > idx {
                // New stream id must be greater than last
                // If not greater, make sure it was not a skipped stream (because its a connection error if it was; stream error if it wasn't)
                if let HTTP2Stream::Closed(closed_stream) = &state.streams[idx]
                    && closed_stream.skipped
                {
                    return Err(HTTP2Error::Connection(HTTP2ErrorCode::StreamClosed));
                }
                state.streams[idx].clone()
            } else {
                if stream_id.is_multiple_of(2) || stream_id <= state.last_stream_id {
                    return Err(HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError));
                }

                state.last_stream_id = stream_id;
                while state.streams.len() < idx {
                    #[allow(clippy::cast_possible_truncation)]
                    state.streams.push(
                        HTTP2StreamClosed {
                            id: state.streams.len() as u32 * 2 + 1,
                            end_stream_received: true,
                            skipped: true,
                        }
                        .into(),
                    );
                }

                state.streams.push(HTTP2Stream::new(stream_id));
                state.streams[idx].clone()
            };

            // Check if the size is greater than max frame size, if so send a GOAWAY and close the connection
            if full_frame_length - 9 > state.settings.max_frame_size as usize {
                println!(
                    "Received frame larger than max frame size, sending GOAWAY and closing connection"
                );
                return Err(HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError));
            }

            match stream.handle_frame(frame, state) {
                Ok((stream_state, frames)) => {
                    state.streams[stream_id.div(2) as usize] = stream_state;
                    Ok(frames)
                }
                Err((stream_state, e)) => {
                    state.streams[stream_id.div(2) as usize] = stream_state;
                    Err(e)
                }
            }
        }
    }
}

fn handle_settings_frame(
    settings_frame: &SettingsFrame,
    state: &mut ConnectionState<'_>,
) -> Result<Vec<Frame>, HTTP2Error> {
    if settings_frame.header.stream_id != 0 {
        return Err(HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError));
    }

    if settings_frame.header.flags.ack {
        state.settings_acked = true;

        return if settings_frame.header.length != 0 {
            Err(HTTP2Error::Connection(HTTP2ErrorCode::FrameSizeError))
        } else {
            Ok(vec![])
        };
    }

    if let Some(initial_window_size) = settings_frame.initial_window_size {
        #[allow(clippy::cast_possible_wrap)]
        let initial_window_size = initial_window_size as i32;
        let delta = initial_window_size - state.settings.window_size;
        state.settings.window_size = initial_window_size;
        // Update all stream window sizes to the new initial window size
        for stream_window in state.stream_window_sizes.values_mut() {
            *stream_window += delta;
        }
    }

    if settings_frame.max_frame_size.is_some() {
        state.settings.max_frame_size = settings_frame.max_frame_size.unwrap();
    }

    let mut ret = vec![];

    if !state.settings_sent {
        let my_settings = SettingsFrameBuilder::new()
            .enable_push(false)
            .header_table_size(4096)
            // .max_concurrent_streams(max) // unlimited
            .initial_window_size(65535)
            .max_frame_size(state.settings.max_frame_size)
            // .max_header_list_size(size) // unlimited
            .build();

        ret.push(my_settings.into());
        state.settings_sent = true;
    }

    ret.push(SettingsFrame::new_ack().into());
    state.settings_acked = false;

    Ok(ret)
}

fn handle_ping_frame(ping_frame: PingFrame) -> Result<Vec<Frame>, HTTP2Error> {
    if ping_frame.header.flags.ack {
        Ok(vec![])
    } else if ping_frame.header.stream_id != 0 || ping_frame.header.length != 8 {
        Err(HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError))
    } else {
        Ok(vec![PingFrame::ack(ping_frame).into()])
    }
}

use std::collections::HashMap;

use hpack::{Decoder, Encoder};

use crate::http2::{
    error::{HTTP2Error, HTTP2ErrorCode, StreamError},
    frames::window_update_frame::WindowUpdateFrame,
    stream::http_stream::HTTP2Stream,
};

pub struct ConnectionSettings {
    pub window_size: i32,
    pub max_frame_size: u32,
}

pub struct ConnectionState<'a> {
    pub decoder: Decoder<'a>,
    pub encoder: Encoder<'a>,
    pub settings_acked: bool,
    pub settings_sent: bool,
    pub settings: ConnectionSettings,
    pub streams: HashMap<u32, HTTP2Stream>,
    pub window_size: i32,
    pub stream_window_sizes: HashMap<u32, i32>, // TODO: this needs to be refactored into the stream struct
}

impl ConnectionState<'_> {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::cast_possible_wrap)]
    pub fn sent_data(&mut self, id: u32, amount: i32) {
        self.window_size -= amount;
        self.stream_window_sizes
            .entry(id)
            .and_modify(|e| *e -= amount);
    }

    #[allow(clippy::cast_possible_wrap)]
    pub fn update_window(&mut self, window_update: &WindowUpdateFrame) -> Result<(), HTTP2Error> {
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

        match window_update.header.stream_id {
            0 => {
                self.window_size = match self
                    .window_size
                    .checked_add(window_update.window_size_increment as i32)
                {
                    Some(new_size) if new_size < 2i32.pow(31) => new_size,
                    _ => {
                        println!(
                            "Updated connection window size would exceed maximum allowed value, sending GOAWAY and closing connection"
                        );
                        return Err(HTTP2Error::Connection(HTTP2ErrorCode::FlowControlError));
                    }
                };
            }
            stream_id => {
                let stream_window = self
                    .stream_window_sizes
                    .entry(stream_id)
                    .or_insert(self.settings.window_size);

                *stream_window =
                    match stream_window.checked_add(window_update.window_size_increment as i32) {
                        Some(new_size) => {
                            #[allow(clippy::cast_sign_loss)]
                            if new_size > 0 && (new_size as u32) >= 2u32.pow(31) {
                                return Err(HTTP2Error::Stream(StreamError {
                                    stream_id,
                                    error_code: HTTP2ErrorCode::FlowControlError,
                                }));
                            }
                            new_size
                        }
                        _ => {
                            return Err(HTTP2Error::Stream(StreamError {
                                stream_id,
                                error_code: HTTP2ErrorCode::FlowControlError,
                            }));
                        }
                    };
            }
        }

        Ok(())
    }
}

impl Default for ConnectionState<'_> {
    fn default() -> Self {
        ConnectionState {
            decoder: Decoder::new(),
            encoder: Encoder::new(),
            settings_acked: true,
            settings_sent: false,
            settings: ConnectionSettings::default(),
            streams: HashMap::new(),
            window_size: 65535,
            stream_window_sizes: HashMap::new(),
        }
    }
}

impl Default for ConnectionSettings {
    fn default() -> Self {
        ConnectionSettings {
            window_size: 65535,
            max_frame_size: 16384,
        }
    }
}

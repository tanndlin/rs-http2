use std::str::FromStr;

use crate::{
    http2::{
        connection_state::ConnectionState,
        error::{HTTP2Error, HTTP2ErrorCode},
        frames::{data_frame::DataFrame, frame::Frame, headers_frame::HeadersFrame},
        header_builder::HeaderBuilder,
        stream::{
            http_stream::HTTP2Stream, stream_closed::HTTP2StreamClosed,
            stream_half_closed_local::HTTP2StreamHalfClosedLocal,
        },
    },
    request::{Method, Request},
    util::handle_request,
};

#[derive(Debug)]
pub struct HTTP2StreamOpen {
    pub id: u32,
    header_builder: HeaderBuilder,
    pending_request: Option<Request>,
}

impl HTTP2StreamOpen {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            header_builder: HeaderBuilder::new(),
            pending_request: None,
        }
    }
    pub fn handle_frame(
        self,
        frame: Frame,
        state: &mut ConnectionState,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        match frame {
            Frame::Data(data_frame) => self.handle_data_frame(state, data_frame),
            Frame::Headers(headers_frame) => self.handle_headers_frame(state, headers_frame),
            _ => todo!(),
        }
    }

    fn handle_data_frame(
        mut self,
        state: &mut ConnectionState,
        data_frame: DataFrame,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        println!("Handling data frame for stream {}", self.id);
        let mut req = match self.pending_request.take() {
            Some(req) => req,
            None => {
                return Err((
                    self.close(false),
                    HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
                ));
            }
        };

        req.body.extend_from_slice(&data_frame.data);
        if data_frame.header.flags.end_stream {
            let res = match handle_request(&req) {
                Ok(res) => res,
                Err(_) => {
                    return Err((
                        self.close(true),
                        HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
                    ));
                }
            };

            dbg!(&res);

            let mut bytes = vec![];
            let headers_frame = HeadersFrame::from((&res, state));
            dbg!(&headers_frame);
            let header_bytes: Vec<u8> = headers_frame.into();
            bytes.extend_from_slice(&header_bytes);
            let data_frame = DataFrame::from(&res);
            dbg!(&data_frame);
            let data_frame_bytes: Vec<u8> = data_frame.into();
            bytes.extend_from_slice(&data_frame_bytes);
            Ok((self.close(true), bytes))
        } else {
            Ok((HTTP2Stream::Open(self), vec![]))
        }
    }

    fn handle_headers_frame(
        mut self,
        state: &mut ConnectionState,
        headers_frame: HeadersFrame,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        println!("Handling headers frame for stream {}", self.id);
        self.header_builder
            .new_fragment(&headers_frame.header_block_fragment);
        if !headers_frame.header.flags.end_headers {
            todo!("Implement continuation headers")
        }
        let end_stream = headers_frame.header.flags.end_stream;

        let headers = match self.header_builder.build(&mut state.decoder) {
            Ok(h) => h,
            Err(_) => {
                return Err((
                    self.close(end_stream),
                    HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
                ));
            }
        };

        // dbg!(&headers);
        let method = match headers.get(":method") {
            Some(method) => method,
            None => {
                return Err((
                    self.close(end_stream),
                    HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
                ));
            }
        };

        let method = match Method::from_str(method) {
            Ok(m) => m,
            Err(_) => {
                return Err((
                    self.close(end_stream),
                    HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
                ));
            }
        };

        let path = match headers.get(":path") {
            Some(path) => path.clone(),
            None => {
                return Err((
                    self.close(end_stream),
                    HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
                ));
            }
        };

        let req = Request {
            headers,
            method,
            path,
            stream_id: self.id,
            body: vec![],
        };

        if !end_stream {
            self.pending_request = Some(req);
            return Ok((HTTP2Stream::Open(self), vec![]));
        }

        let res = match handle_request(&req) {
            Ok(res) => res,
            Err(_) => {
                return Err((
                    self.close(end_stream),
                    HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
                ));
            }
        };

        let mut bytes = vec![];
        let headers_frame = HeadersFrame::from((&res, state));
        let header_bytes: Vec<u8> = headers_frame.into();
        bytes.extend_from_slice(&header_bytes);
        let data_frame = DataFrame::from(&res);
        let data_frame_bytes: Vec<u8> = data_frame.into();
        bytes.extend_from_slice(&data_frame_bytes);
        Ok((self.close(end_stream), bytes))
    }

    pub fn close(self, end_stream: bool) -> HTTP2Stream {
        println!("Closing stream: {}", self.id);
        HTTP2Stream::Closed(HTTP2StreamClosed::new(self.id, end_stream))
    }

    pub fn half_close_local(self) -> HTTP2Stream {
        println!("Half-closing stream locally: {}", self.id);
        HTTP2Stream::HalfClosedLocal(HTTP2StreamHalfClosedLocal { id: self.id })
    }

    pub fn half_close_remote(self) -> HTTP2Stream {
        println!("Half-closing stream remotely: {}", self.id);
        HTTP2Stream::HalfClosedRemote(
            crate::http2::stream::stream_half_closed_remote::HTTP2StreamHalfClosedRemote {
                id: self.id,
            },
        )
    }
}

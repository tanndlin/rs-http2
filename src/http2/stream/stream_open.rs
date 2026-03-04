use std::str::FromStr;

use crate::{
    handle_request,
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
};

pub struct HTTP2StreamOpen {
    pub id: u32,
    header_builder: HeaderBuilder,
}

impl HTTP2StreamOpen {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            header_builder: HeaderBuilder::new(),
        }
    }
    pub fn handle_frame(
        mut self,
        frame: Frame,
        state: &mut ConnectionState,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        let stream_id = self.id;

        match frame {
            Frame::Headers(headers_frame) => {
                self.handle_headers_frame(state, stream_id, headers_frame)
            }
            _ => todo!(),
        }
    }

    fn handle_headers_frame(
        mut self,
        state: &mut ConnectionState,
        stream_id: u32,
        headers_frame: HeadersFrame,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        self.header_builder
            .new_fragment(&headers_frame.header_block_fragment);
        if !headers_frame.header.flags.end_headers {
            todo!("Implement continuation headers")
        }
        let headers = self.header_builder.build(&mut state.decoder).map_err(|_| {
            println!("failed to decode headers");
            (
                HTTP2Stream::Closed(HTTP2StreamClosed { id: stream_id }),
                HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
            )
        })?;
        // TODO
        dbg!(&headers);
        let method = headers.get(":method").ok_or_else(|| {
            println!("couldnt get method from headers");
            (
                HTTP2Stream::Closed(HTTP2StreamClosed { id: stream_id }),
                HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
            )
        })?;
        let method = Method::from_str(method).map_err(|_| {
            println!("failed to parse method");
            (
                HTTP2Stream::Closed(HTTP2StreamClosed { id: stream_id }),
                HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
            )
        })?;
        let path = headers
            .get(":path")
            .ok_or_else(|| {
                println!("couldnt get path from headers");
                (
                    HTTP2Stream::Closed(HTTP2StreamClosed { id: stream_id }),
                    HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
                )
            })?
            .clone();
        let req = Request {
            headers,
            method,
            path,
            stream_id: self.id,
        };
        let res = handle_request(&req).map_err(|_| {
            (
                HTTP2Stream::Closed(HTTP2StreamClosed { id: stream_id }),
                HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
            )
        })?;
        let mut bytes = vec![];
        let headers_frame = HeadersFrame::from((&res, state));
        let header_bytes: Vec<u8> = headers_frame.into();
        bytes.extend_from_slice(&header_bytes);
        let data_frame = DataFrame::from(&res);
        let data_frame_bytes: Vec<u8> = data_frame.into();
        bytes.extend_from_slice(&data_frame_bytes);
        Ok((self.close(), bytes))
    }

    pub fn close(self) -> HTTP2Stream {
        HTTP2Stream::Closed(HTTP2StreamClosed { id: self.id })
    }

    pub fn half_close_local(self) -> HTTP2Stream {
        HTTP2Stream::HalfClosedLocal(HTTP2StreamHalfClosedLocal { id: self.id })
    }
}

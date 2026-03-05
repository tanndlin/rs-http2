use std::str::FromStr;

use crate::{
    encode_to::EncodeTo,
    http2::{
        connection_state::ConnectionState,
        error::{HTTP2Error, HTTP2ErrorCode, StreamError},
        frames::{
            continuation_frame::ContinuationFrame, data_frame::DataFrame, frame::Frame,
            headers_frame::HeadersFrame, priority_frame::PriorityFrame,
        },
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
    pending_request: Option<Box<Request>>,
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
            Frame::Continuation(continuation_frame) => {
                self.handle_continuation_frame(state, continuation_frame)
            }
            Frame::Priority(priority_frame) => self.handle_priority_frame(&priority_frame),
            _ => todo!(),
        }
    }

    fn handle_data_frame(
        mut self,
        state: &mut ConnectionState,
        data_frame: DataFrame,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        println!("Handling data frame for stream {}", self.id);
        let Some(mut req) = self.pending_request.take() else {
            return Err((
                self.close(false),
                HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
            ));
        };

        req.body.extend(data_frame.data);
        if data_frame.header.flags.end_stream {
            let Some(res) = handle_request(&req).ok() else {
                return Err((
                    self.close(true),
                    HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
                ));
            };
            dbg!(&res);

            let headers_frame = HeadersFrame::from((&res, state));
            dbg!(&headers_frame);
            let data_frame = DataFrame::from(&res);
            dbg!(&data_frame);

            let mut bytes = headers_frame.to_bytes();
            data_frame.encode_to(&mut bytes);
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
            .new_fragment(headers_frame.header_block_fragment);
        if !headers_frame.header.flags.end_headers {
            return Ok((HTTP2Stream::Open(self), vec![]));
        }

        if let Some(dep) = headers_frame.stream_dependency
            && dep == headers_frame.header.stream_id
        {
            return Err((
                self.close(true),
                HTTP2Error::Stream(StreamError {
                    stream_id: headers_frame.header.stream_id,
                    error_code: HTTP2ErrorCode::ProtocolError,
                }),
            ));
        }

        let end_stream = headers_frame.header.flags.end_stream;

        let headers = match self.header_builder.build(&mut state.decoder) {
            Ok(headers) => headers,
            Err(e) => {
                println!("Error building headers: {e:?}");
                return Err((self.close(true), e));
            }
        };

        let Some(method) = headers.get(":method") else {
            return Err((
                self.close(end_stream),
                HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
            ));
        };

        let Ok(method) = Method::from_str(method) else {
            return Err((
                self.close(end_stream),
                HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
            ));
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
            self.pending_request = Some(Box::new(req));
            return Ok((HTTP2Stream::Open(self), vec![]));
        }

        let Ok(res) = handle_request(&req) else {
            return Err((
                self.close(end_stream),
                HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
            ));
        };

        let headers_frame = HeadersFrame::from((&res, state));
        let data_frame = DataFrame::from(&res);
        let mut bytes = headers_frame.to_bytes();
        data_frame.encode_to(&mut bytes);
        Ok((self.close(end_stream), bytes))
    }

    fn handle_continuation_frame(
        mut self,
        state: &mut ConnectionState,
        continuation_frame: ContinuationFrame,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        println!("Handling continuation frame for stream {}", self.id);
        self.header_builder
            .new_fragment(continuation_frame.header_block_fragment);
        if !continuation_frame.header.flags.end_headers {
            return Ok((HTTP2Stream::Open(self), vec![]));
        }

        let headers = match self.header_builder.build(&mut state.decoder) {
            Ok(headers) => headers,
            Err(e) => {
                println!("Error building headers: {e:?}");
                return Err((self.close(true), e));
            }
        };

        let Some(method) = headers.get(":method") else {
            return Err((
                self.close(false),
                HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
            ));
        };

        let Ok(method) = Method::from_str(method) else {
            return Err((
                self.close(false),
                HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
            ));
        };

        let path = match headers.get(":path") {
            Some(path) => path.clone(),
            None => {
                return Err((
                    self.close(false),
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

        let Ok(res) = handle_request(&req) else {
            return Err((
                self.close(true),
                HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
            ));
        };

        let headers_frame = HeadersFrame::from((&res, state));
        let data_frame = DataFrame::from(&res);
        let mut bytes = headers_frame.to_bytes();
        data_frame.encode_to(&mut bytes);
        Ok((self.close(true), bytes))
    }

    fn handle_priority_frame(
        self,
        priority_frame: &PriorityFrame,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        let id = self.id;
        println!("Got priority frame for stream {id}");

        if priority_frame.stream_dependency == id {
            return Err((
                self.close(true),
                HTTP2Error::Stream(StreamError {
                    stream_id: priority_frame.header.stream_id,
                    error_code: HTTP2ErrorCode::ProtocolError,
                }),
            ));
        }

        Ok((HTTP2Stream::Open(self), vec![]))
    }

    pub fn waiting_for_continuation(&self) -> bool {
        self.header_builder.waiting_for_continuation()
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

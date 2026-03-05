use crate::http2::{
    error::{HTTP2Error, HTTP2ErrorCode, StreamError},
    frames::{frame::Frame, priority_frame::PriorityFrame},
    stream::http_stream::HTTP2Stream,
};

#[derive(Debug)]
pub struct HTTP2StreamClosed {
    pub id: u32,
    end_stream_received: bool,
}

impl HTTP2StreamClosed {
    pub fn handle_frame(
        self,
        frame: &Frame,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        match frame {
            Frame::Priority(p) => self.handle_priority_frame(p),
            _ => {
                let id = self.id;
                match self.end_stream_received {
                    true => Err((
                        HTTP2Stream::Closed(self),
                        HTTP2Error::Connection(HTTP2ErrorCode::StreamClosed),
                    )),
                    false => Err((
                        HTTP2Stream::Closed(self),
                        HTTP2Error::Stream(StreamError::new(id, HTTP2ErrorCode::StreamClosed)),
                    )),
                }
            }
        }
    }

    fn handle_priority_frame(
        self,
        priority_frame: &PriorityFrame,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        let id = self.id;
        println!("Got priority frame for stream {id}");

        if priority_frame.stream_dependency == id {
            return Err((
                HTTP2Stream::Closed(self),
                HTTP2Error::Stream(StreamError {
                    stream_id: priority_frame.header.stream_id,
                    error_code: HTTP2ErrorCode::ProtocolError,
                }),
            ));
        }

        Ok((HTTP2Stream::Closed(self), vec![]))
    }

    pub fn new(id: u32, end_stream_received: bool) -> Self {
        Self {
            id,
            end_stream_received,
        }
    }
}

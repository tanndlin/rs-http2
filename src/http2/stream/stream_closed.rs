use crate::http2::{
    error::{HTTP2Error, HTTP2ErrorCode, StreamError},
    frames::frame::Frame,
    stream::http_stream::HTTP2Stream,
};

pub struct HTTP2StreamClosed {
    pub id: u32,
    end_stream_received: bool,
}

impl HTTP2StreamClosed {
    pub fn handle_frame(
        self,
        frame: Frame,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        match frame {
            Frame::Priority(_) => todo!(),
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

    pub fn new(id: u32, end_stream_received: bool) -> Self {
        Self {
            id,
            end_stream_received,
        }
    }
}

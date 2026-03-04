use crate::http2::{
    error::{HTTP2Error, HTTP2ErrorCode, StreamError},
    frames::frame::Frame,
    stream::http_stream::HTTP2Stream,
};

pub struct HTTP2StreamClosed {
    pub id: u32,
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
                Err((
                    self.close(),
                    HTTP2Error::Stream(StreamError::new(id, HTTP2ErrorCode::StreamClosed)),
                ))
            }
        }
    }

    pub fn close(self) -> HTTP2Stream {
        HTTP2Stream::Closed(HTTP2StreamClosed { id: self.id })
    }
}

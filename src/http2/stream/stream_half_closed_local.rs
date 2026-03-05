use crate::http2::{
    error::{HTTP2Error, HTTP2ErrorCode, StreamError},
    frames::{frame::Frame, priority_frame::PriorityFrame},
    stream::http_stream::HTTP2Stream,
};

#[derive(Debug)]
pub struct HTTP2StreamHalfClosedLocal {
    pub id: u32,
}

impl HTTP2StreamHalfClosedLocal {
    pub fn handle_frame(
        self,
        frame: &Frame,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        match frame {
            Frame::Priority(p) => self.handle_priority_frame(p),
            _ => todo!(),
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
                HTTP2Stream::HalfClosedLocal(self),
                HTTP2Error::Stream(StreamError {
                    stream_id: priority_frame.header.stream_id,
                    error_code: HTTP2ErrorCode::ProtocolError,
                }),
            ));
        }

        Ok((HTTP2Stream::HalfClosedLocal(self), vec![]))
    }
}

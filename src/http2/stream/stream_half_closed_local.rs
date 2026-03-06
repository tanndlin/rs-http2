use crate::http2::{
    connection_state::ConnectionState,
    error::{HTTP2Error, HTTP2ErrorCode, StreamError},
    frames::{frame::Frame, priority_frame::PriorityFrame},
    stream::{http_stream::HTTP2Stream, stream_closed::HTTP2StreamClosed},
};

#[derive(Clone, Debug)]
pub struct HTTP2StreamHalfClosedLocal {
    pub id: u32,
}

impl HTTP2StreamHalfClosedLocal {
    pub fn handle_frame(
        self,
        frame: &Frame,
        state: &mut ConnectionState<'_>,
    ) -> Result<(HTTP2Stream, Vec<Frame>), (HTTP2Stream, HTTP2Error)> {
        match frame {
            Frame::Priority(p) => self.handle_priority_frame(p),
            Frame::WindowUpdate(window_update) => {
                if let Err(e) = state.update_window(window_update) {
                    println!("Error updating window: {e:?}");
                    return Err((self.close(false), e));
                }
                Ok((self.into(), vec![]))
            }
            _ => todo!(),
        }
    }

    fn handle_priority_frame(
        self,
        priority_frame: &PriorityFrame,
    ) -> Result<(HTTP2Stream, Vec<Frame>), (HTTP2Stream, HTTP2Error)> {
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

    fn close(self, end_stream: bool) -> HTTP2Stream {
        println!("Closing stream: {}", self.id);
        HTTP2StreamClosed::new(self.id, end_stream).into()
    }
}

impl From<HTTP2StreamHalfClosedLocal> for HTTP2Stream {
    fn from(s: HTTP2StreamHalfClosedLocal) -> Self {
        HTTP2Stream::HalfClosedLocal(s)
    }
}

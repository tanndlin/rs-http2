use crate::http2::{
    connection_state::ConnectionState,
    error::{HTTP2Error, HTTP2ErrorCode, StreamError},
    frames::{frame::Frame, priority_frame::PriorityFrame},
    stream::{http_stream::HTTP2Stream, stream_closed::HTTP2StreamClosed},
};

#[derive(Clone, Debug)]
pub struct HTTP2StreamHalfClosedRemote {
    pub id: u32,
}
impl HTTP2StreamHalfClosedRemote {
    pub fn handle_frame(
        self,
        frame: Frame,
        state: &mut ConnectionState<'_>,
    ) -> Result<(HTTP2Stream, Vec<Frame>), (HTTP2Stream, HTTP2Error)> {
        match frame {
            Frame::Priority(p) => self.handle_priority_frame(&p),
            Frame::WindowUpdate(window_update) => {
                if let Err(e) = state.update_window(&window_update) {
                    println!("Error updating window: {e:?}");
                    return Err((self.close(true), e));
                }
                Ok((self.into(), vec![]))
            }
            Frame::RstStream(rst) => {
                println!(
                    "Received RST_STREAM for stream {}, closing stream. Reason: {:?}",
                    rst.header.stream_id,
                    HTTP2ErrorCode::try_from(rst.error_code).unwrap(),
                );
                Ok((self.close(true), vec![]))
            }
            _ => Err((
                self.into(),
                HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
            )),
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
                HTTP2Stream::HalfClosedRemote(self),
                HTTP2Error::Stream(StreamError {
                    stream_id: priority_frame.header.stream_id,
                    error_code: HTTP2ErrorCode::ProtocolError,
                }),
            ));
        }

        Ok((HTTP2Stream::HalfClosedRemote(self), vec![]))
    }

    fn close(self, end_stream: bool) -> HTTP2Stream {
        println!("Closing stream: {}", self.id);
        HTTP2StreamClosed::new(self.id, end_stream).into()
    }
}

impl From<HTTP2StreamHalfClosedRemote> for HTTP2Stream {
    fn from(s: HTTP2StreamHalfClosedRemote) -> Self {
        HTTP2Stream::HalfClosedRemote(s)
    }
}

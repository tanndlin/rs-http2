use crate::http2::{
    connection_state::ConnectionState,
    error::{HTTP2Error, HTTP2ErrorCode, StreamError},
    frames::{frame::Frame, headers_frame::HeadersFrame},
    stream::{
        http_stream::HTTP2Stream, stream_closed::HTTP2StreamClosed, stream_open::HTTP2StreamOpen,
    },
};

pub struct HTTP2StreamIdle {
    pub id: u32,
}

impl HTTP2StreamIdle {
    pub fn handle_frame(
        self,
        frame: Frame,
        state: &mut ConnectionState,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        match frame {
            Frame::Headers(headers_frame) => self.handle_headers_frame(headers_frame, state),
            Frame::Priority(priority_frame) => todo!(),
            Frame::PushPromise(push_promise_frame) => todo!(),
            _ => {
                println!("Got non-header/priority frame in idle state");
                Err((
                    self.close(false), // TODO: Check whether this should be true or false
                    HTTP2Error::Connection(HTTP2ErrorCode::ProtocolError),
                ))
            }
        }
    }

    pub fn handle_headers_frame(
        self,
        headers_frame: HeadersFrame,
        state: &mut ConnectionState,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        let ret_state = self.open();
        ret_state.handle_frame(Frame::Headers(headers_frame), state)
    }

    pub fn close(self, end_stream: bool) -> HTTP2Stream {
        HTTP2Stream::Closed(HTTP2StreamClosed::new(self.id, end_stream))
    }

    pub fn open(self) -> HTTP2Stream {
        println!("Opening stream: {}", self.id);
        HTTP2Stream::Open(HTTP2StreamOpen::new(self.id))
    }
}

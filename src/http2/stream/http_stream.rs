use crate::http2::{
    connection_state::ConnectionState,
    error::HTTP2Error,
    frames::frame::Frame,
    stream::{
        stream_closed::HTTP2StreamClosed, stream_half_closed_local::HTTP2StreamHalfClosedLocal,
        stream_half_closed_remote::HTTP2StreamHalfClosedRemote, stream_idle::HTTP2StreamIdle,
        stream_open::HTTP2StreamOpen,
    },
};

#[derive(Debug)]
pub enum HTTP2Stream {
    Idle(HTTP2StreamIdle),
    Open(HTTP2StreamOpen),
    ReservedLocal,
    ReservedRemote,
    HalfClosedRemote(HTTP2StreamHalfClosedRemote),
    HalfClosedLocal(HTTP2StreamHalfClosedLocal),
    Closed(HTTP2StreamClosed),
}

impl HTTP2Stream {
    pub fn handle_frame(
        self,
        frame: Frame,
        state: &mut ConnectionState,
    ) -> Result<(HTTP2Stream, Vec<Frame>), (HTTP2Stream, HTTP2Error)> {
        match self {
            HTTP2Stream::Idle(stream) => stream.handle_frame(frame, state),
            HTTP2Stream::Open(stream) => stream.handle_frame(frame, state),
            HTTP2Stream::ReservedLocal => todo!(),
            HTTP2Stream::ReservedRemote => todo!(),
            HTTP2Stream::HalfClosedRemote(stream) => stream.handle_frame(frame, state),
            HTTP2Stream::HalfClosedLocal(stream) => stream.handle_frame(&frame, state),
            HTTP2Stream::Closed(stream) => stream.handle_frame(&frame),
        }
    }
}

impl HTTP2Stream {
    pub fn new(id: u32) -> Self {
        HTTP2Stream::Idle(HTTP2StreamIdle { id })
    }
}

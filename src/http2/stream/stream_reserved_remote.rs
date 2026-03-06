use crate::http2::{
    connection_state::ConnectionState, error::HTTP2Error, frames::frame::Frame,
    stream::http_stream::HTTP2Stream,
};

#[derive(Clone, Debug)]
pub struct HTTP2StreamReservedRemote {
    pub id: u32,
}

impl HTTP2StreamReservedRemote {
    pub fn new(id: u32) -> Self {
        Self { id }
    }

    pub fn handle_frame(
        self,
        frame: Frame,
        state: &mut ConnectionState<'_>,
    ) -> Result<(HTTP2Stream, Vec<Frame>), (HTTP2Stream, HTTP2Error)> {
        match frame {
            _ => todo!("Implement handle_frame for reserved (remote) stream"),
        }
    }
}

impl From<HTTP2StreamReservedRemote> for HTTP2Stream {
    fn from(stream: HTTP2StreamReservedRemote) -> Self {
        HTTP2Stream::ReservedRemote(stream)
    }
}

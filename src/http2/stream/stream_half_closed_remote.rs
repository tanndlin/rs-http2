use crate::http2::{error::HTTP2Error, frames::frame::Frame, stream::http_stream::HTTP2Stream};

#[derive(Debug)]
pub struct HTTP2StreamHalfClosedRemote {
    pub id: u32,
}
impl HTTP2StreamHalfClosedRemote {
    pub fn handle_frame(
        self,
        frame: Frame,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        todo!()
    }
}

use crate::http2::{error::HTTP2Error, frames::frame::Frame, stream::http_stream::HTTP2Stream};

pub struct HTTP2StreamHalfClosedLocal {
    pub id: u32,
}
impl HTTP2StreamHalfClosedLocal {
    pub fn handle_frame(
        &self,
        frame: Frame,
    ) -> Result<(HTTP2Stream, Vec<u8>), (HTTP2Stream, HTTP2Error)> {
        todo!()
    }
}

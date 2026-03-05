use crate::{
    encode_to::EncodeTo,
    http2::{error::HTTP2Error, frames::frame::FrameHeader},
};

#[derive(Debug)]
pub struct WindowUpdateFrame {
    pub header: FrameHeader<u8>,
    pub window_size_increment: u32, // 31 bits, so the most significant bit must be ignored
}

impl TryFrom<&[u8]> for WindowUpdateFrame {
    type Error = HTTP2Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let header = FrameHeader::<u8>::try_from(buf)?;
        let window_size_increment =
            u32::from_be_bytes(buf[9..13].try_into().unwrap()) & 0x7FFF_FFFF; // mask out the most significant bit

        Ok(Self {
            header,
            window_size_increment,
        })
    }
}

impl EncodeTo for WindowUpdateFrame {
    fn encode_to(self, buf: &mut Vec<u8>) {
        self.header.encode_to(buf);
        buf.extend((self.window_size_increment & 0x7FFF_FFFF).to_be_bytes());
    }
}

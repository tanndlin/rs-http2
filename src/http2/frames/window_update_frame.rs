use crate::http2::frames::frame::FrameHeader;

#[derive(Debug)]
pub struct WindowUpdateFrame {
    pub header: FrameHeader<u8>,
    pub window_size_increment: u32, // 31 bits, so the most significant bit must be ignored
}

impl TryFrom<&[u8]> for WindowUpdateFrame {
    type Error = String;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let header = FrameHeader::<u8>::try_from(buf)?;
        let window_size_increment =
            u32::from_be_bytes(buf[9..13].try_into().map_err(|_| "Invalid data length")?)
                & 0x7FFFFFFF; // mask out the most significant bit

        Ok(Self {
            header,
            window_size_increment,
        })
    }
}

impl From<WindowUpdateFrame> for Vec<u8> {
    fn from(frame: WindowUpdateFrame) -> Self {
        let mut ret = vec![];
        let header_bytes: Vec<u8> = frame.header.into();
        ret.extend_from_slice(&header_bytes);
        ret.extend_from_slice(&(frame.window_size_increment & 0x7FFFFFFF).to_be_bytes());
        ret
    }
}

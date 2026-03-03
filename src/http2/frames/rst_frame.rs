use crate::http2::frames::frame::{FrameHeader, FrameType};

#[derive(Debug)]
pub struct RstFrame {
    pub header: FrameHeader<u8>,
    pub error_code: u32,
}

impl RstFrame {
    pub fn new(stream_id: u32, error_code: u32) -> Self {
        Self {
            header: FrameHeader {
                length: 8,
                frame_type: FrameType::RstStream,
                flags: 0,
                stream_id,
            },
            error_code,
        }
    }
}

impl TryFrom<&[u8]> for RstFrame {
    type Error = String;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let header = FrameHeader::try_from(buf)?;
        let error_code =
            u32::from_be_bytes(buf[9..13].try_into().map_err(|_| "Invalid data length")?);

        Ok(Self { header, error_code })
    }
}

impl From<RstFrame> for Vec<u8> {
    fn from(frame: RstFrame) -> Self {
        let mut ret = vec![];
        let header_bytes: Vec<u8> = frame.header.into();
        ret.extend_from_slice(&header_bytes);
        ret.extend(frame.error_code.to_be_bytes());
        ret
    }
}

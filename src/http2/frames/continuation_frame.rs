use crate::http2::{error::HTTP2Error, frames::frame::FrameHeader};

#[derive(Debug)]
pub struct ContinuationFrameFlags {
    pub end_headers: bool, // bit 2
}

impl From<u8> for ContinuationFrameFlags {
    fn from(value: u8) -> Self {
        let end_headers = (value & 4) > 0;

        Self { end_headers }
    }
}

#[derive(Debug)]
pub struct ContinuationFrame {
    pub header: FrameHeader<ContinuationFrameFlags>,
    pub header_block_fragment: Vec<u8>,
}

impl TryFrom<&[u8]> for ContinuationFrame {
    type Error = HTTP2Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let header = FrameHeader::<ContinuationFrameFlags>::try_from(buf)?;
        let header_block_fragment = buf[9..header.length as usize].to_vec();

        Ok(Self {
            header,
            header_block_fragment,
        })
    }
}

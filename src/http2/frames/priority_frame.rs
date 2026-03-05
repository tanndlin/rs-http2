use crate::http2::{error::HTTP2Error, frames::frame::FrameHeader};

#[derive(Debug)]
pub struct PriorityFrame {
    pub header: FrameHeader<u8>,
    pub exclusive: bool,        // 1 bit
    pub stream_dependency: u32, // 31 bits
    pub weight: u8,             // 8 bits
}

impl TryFrom<&[u8]> for PriorityFrame {
    type Error = HTTP2Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let header = FrameHeader::try_from(buf)?;
        let n = u32::from_be_bytes(buf[9..13].try_into().unwrap());
        let exclusive = (n & (1 << 31)) > 0;
        let stream_dependency = n & !(1 << 31);
        let weight = buf[13];

        Ok(Self {
            header,
            exclusive,
            stream_dependency,
            weight,
        })
    }
}

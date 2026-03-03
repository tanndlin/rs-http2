use crate::http2::frames::frame::{FrameHeader, FrameType};

#[derive(Debug)]
pub struct PingFrameFlags {
    pub ack: bool,
}

impl From<u8> for PingFrameFlags {
    fn from(value: u8) -> Self {
        let ack = (value & 1) > 0;

        Self { ack }
    }
}

impl From<PingFrameFlags> for u8 {
    fn from(flags: PingFrameFlags) -> Self {
        if flags.ack { 1 } else { 0 }
    }
}

#[derive(Debug)]
pub struct PingFrame {
    pub header: FrameHeader<PingFrameFlags>,
    data: [u8; 8],
}

impl PingFrame {
    pub fn ack() -> Self {
        Self {
            header: FrameHeader {
                length: 8,
                frame_type: FrameType::Ping,
                flags: PingFrameFlags { ack: true },
                stream_id: 0,
            },
            data: [0; 8],
        }
    }
}

impl TryFrom<&[u8]> for PingFrame {
    type Error = String;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let header = FrameHeader::try_from(buf)?;
        let data = buf[9..17].try_into().map_err(|_| "Invalid data length")?;

        Ok(Self { header, data })
    }
}

impl From<PingFrame> for Vec<u8> {
    fn from(frame: PingFrame) -> Self {
        let mut ret = vec![];
        let header_bytes: Vec<u8> = frame.header.into();
        ret.extend_from_slice(&header_bytes);
        ret.extend(frame.data);
        ret
    }
}

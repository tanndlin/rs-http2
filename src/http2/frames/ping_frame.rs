use crate::{
    encode_to::EncodeTo,
    http2::{
        error::HTTP2Error,
        frames::frame::{FrameHeader, FrameType},
    },
};

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
        u8::from(flags.ack)
    }
}

#[derive(Debug)]
pub struct PingFrame {
    pub header: FrameHeader<PingFrameFlags>,
    pub data: [u8; 8],
}

impl PingFrame {
    pub fn ack(other: PingFrame) -> Self {
        Self {
            header: FrameHeader {
                length: 8,
                frame_type: FrameType::Ping,
                flags: PingFrameFlags { ack: true },
                stream_id: 0,
            },
            data: other.data,
        }
    }
}

impl TryFrom<&[u8]> for PingFrame {
    type Error = HTTP2Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let header = FrameHeader::try_from(buf)?;
        let data = buf[9..17].try_into().unwrap();

        Ok(Self { header, data })
    }
}

impl EncodeTo for PingFrame {
    fn encode_to(self, buf: &mut Vec<u8>) {
        self.header.encode_to(buf);
        buf.extend(self.data);
    }
}

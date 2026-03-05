use crate::{
    encode_to::EncodeTo,
    http2::{
        error::{HTTP2Error, HTTP2ErrorCode, StreamError},
        frames::frame::{FrameHeader, FrameType},
    },
};

#[derive(Debug)]
pub struct RstFrame {
    pub header: FrameHeader<u8>,
    pub error_code: u32,
}

impl RstFrame {
    pub fn new(stream_id: u32, e: HTTP2ErrorCode) -> Self {
        Self {
            header: FrameHeader {
                length: 4,
                frame_type: FrameType::RstStream,
                flags: 0,
                stream_id,
            },
            error_code: e as u32,
        }
    }
}

impl From<StreamError> for RstFrame {
    fn from(e: StreamError) -> Self {
        let StreamError {
            stream_id,
            error_code,
        } = e;

        RstFrame::new(stream_id, error_code)
    }
}

impl TryFrom<&[u8]> for RstFrame {
    type Error = HTTP2Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let header = FrameHeader::try_from(buf)?;
        let error_code = u32::from_be_bytes(buf[9..13].try_into().unwrap());

        Ok(Self { header, error_code })
    }
}

impl EncodeTo for RstFrame {
    fn encode_to(self, buf: &mut Vec<u8>) {
        self.header.encode_to(buf);
        buf.extend(self.error_code.to_be_bytes());
    }
}

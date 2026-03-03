use crate::http2::frames::frame::FrameHeader;

#[derive(Debug)]
pub struct GoAwayFrameFlags {}

impl From<u8> for GoAwayFrameFlags {
    fn from(value: u8) -> Self {
        todo!()
    }
}

#[derive(Debug)]
pub struct GoAwayFrame {
    header: FrameHeader<GoAwayFrameFlags>,
    last_stream_id: u32, // 31 bits
    error_code: u32,
    data: Vec<u8>,
}

impl TryFrom<&[u8]> for GoAwayFrame {
    type Error = String;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let header = FrameHeader::<GoAwayFrameFlags>::try_from(buf)?;
        let n = u32::from_be_bytes(buf[9..13].try_into().map_err(|_| "Not enough bytes")?);
        let last_stream_id = n & !(1 << 31);
        let error_code =
            u32::from_be_bytes(buf[13..17].try_into().map_err(|_| "Not enough bytes")?);

        let data = buf[17..17 + header.length as usize].to_vec();

        Ok(Self {
            header,
            last_stream_id,
            error_code,
            data,
        })
    }
}

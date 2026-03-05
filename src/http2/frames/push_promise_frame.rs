use crate::http2::{error::HTTP2Error, frames::frame::FrameHeader};

#[derive(Debug)]
pub struct PushPromiseFrameFlags {
    pub end_headers: bool,
    pub padded: bool,
}

impl From<u8> for PushPromiseFrameFlags {
    fn from(bits: u8) -> Self {
        let end_headers = (bits & 4) > 0;
        let padded = (bits & 8) > 0;

        Self {
            end_headers,
            padded,
        }
    }
}

#[derive(Debug)]
pub struct PushPromiseFrame {
    pub header: FrameHeader<PushPromiseFrameFlags>,
    pad_length: u8,
    stream_id: u32, // 31 bits
    header_block_fragment: Vec<u8>,
}

impl TryFrom<&[u8]> for PushPromiseFrame {
    type Error = HTTP2Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let mut buf = buf;

        let header = FrameHeader::<PushPromiseFrameFlags>::try_from(buf)?;
        let mut frag_length = header.length as usize;
        buf = &buf[9..];
        let pad_length = if header.flags.padded {
            let val = buf[0];
            buf = &buf[1..];
            frag_length -= 1 + val as usize;
            val
        } else {
            0
        };

        let n = u32::from_be_bytes(buf[0..4].try_into().unwrap());
        let stream_id = n & !(1 << 31);
        buf = &buf[4..];
        let header_block_fragment = buf[..frag_length].to_vec();

        Ok(Self {
            header,
            pad_length,
            stream_id,
            header_block_fragment,
        })
    }
}

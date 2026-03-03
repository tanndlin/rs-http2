use std::io::Read;

use crate::{
    http2::{
        connection_state::ConnectionState,
        frames::frame::{FrameHeader, FrameType},
    },
    response::Response,
};

#[derive(Debug)]
pub struct HeadersFrameFlags {
    pub end_stream: bool,  // bit 0
    pub end_headers: bool, // bit 2
    padded: bool,          // bit 3
    pub priority: bool,    //bit 5
}

impl From<u8> for HeadersFrameFlags {
    fn from(bits: u8) -> Self {
        Self {
            end_stream: bits & 1 > 0,
            end_headers: bits & 4 > 0,
            padded: bits & 8 > 0,
            priority: bits & 32 > 0,
        }
    }
}

impl From<HeadersFrameFlags> for u8 {
    fn from(flags: HeadersFrameFlags) -> Self {
        let mut bits = 0u8;
        bits |= flags.end_stream as u8;
        bits |= (flags.end_headers as u8) << 2;
        bits |= (flags.padded as u8) << 3;
        bits |= (flags.priority as u8) << 5;
        bits
    }
}

#[derive(Debug)]
pub struct HeadersFrame {
    pub header: FrameHeader<HeadersFrameFlags>,
    pad_length: u8,
    exclusive: Option<bool>,
    stream_dependency: Option<u32>, // 31 bits
    weight: Option<u8>,
    pub header_block_fragment: Vec<u8>,
}

impl TryFrom<&[u8]> for HeadersFrame {
    type Error = String;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let mut buf = buf;
        let header: FrameHeader<HeadersFrameFlags> = FrameHeader::try_from(buf)?;
        dbg!(&header);

        if header.stream_id == 0 {
            return Err("HEADERS Frame stream identifier cannot be zero".to_string());
        }

        buf = &buf[9..];
        let mut frag_length = (header.length) as usize;
        let pad_length = if header.flags.padded {
            let val = buf[0];
            buf = &buf[1..];
            frag_length -= 1 + val as usize;
            val
        } else {
            0
        };

        let (exclusive, stream_dependency, weight) = if header.flags.priority {
            let mask = 1u32 << 31;
            let value = u32::from_be_bytes(buf[..4].try_into().unwrap());
            let weight = buf[4];
            frag_length -= 5;
            buf = &buf[5..];

            let exclusive = value & mask > 0;
            let stream_dependency = value & !mask;
            (Some(exclusive), Some(stream_dependency), Some(weight))
        } else {
            (None, None, None)
        };

        let mut header_block_fragment = vec![0; frag_length];
        buf.read_exact(&mut header_block_fragment).map_err(|_| {
            format!(
                "HeaderFrame buffer had less than {frag_length} bytes for header block fragment"
            )
        })?;

        Ok(Self {
            header,
            pad_length,
            exclusive,
            stream_dependency,
            weight,
            header_block_fragment,
        })
    }
}

impl<'a> From<(&Response, &mut ConnectionState<'a>)> for HeadersFrame {
    fn from(pair: (&Response, &mut ConnectionState)) -> Self {
        let (res, state) = pair;
        let mut bytes: Vec<(Vec<u8>, Vec<u8>)> = vec![];

        let binding = res.status_code.to_code().to_string();
        dbg!(&res.status_code);
        bytes.push((":status".as_bytes().to_vec(), binding.as_bytes().to_vec()));

        dbg!(&res.headers);
        for (name, value) in &res.headers {
            let lower = name.to_lowercase();
            bytes.push((lower.into_bytes(), value.as_bytes().to_vec()));
        }

        let encoded = state
            .encoder
            .encode(bytes.iter().map(|(k, v)| (k.as_slice(), v.as_slice())));
        let header = FrameHeader::<HeadersFrameFlags> {
            length: encoded.len() as u32,
            frame_type: FrameType::Headers,
            flags: HeadersFrameFlags {
                end_stream: false,
                end_headers: true,
                padded: false,
                priority: false,
            },
            stream_id: res.stream_id,
        };

        HeadersFrame {
            header,
            pad_length: 0,
            exclusive: None,
            stream_dependency: None,
            weight: None,
            header_block_fragment: encoded,
        }
    }
}

impl From<HeadersFrame> for Vec<u8> {
    fn from(headers_frame: HeadersFrame) -> Self {
        let mut payload = vec![];

        if headers_frame.header.flags.padded {
            payload.push(headers_frame.pad_length);
        }

        if headers_frame.header.flags.priority {
            payload.extend_from_slice(
                &(headers_frame.stream_dependency.unwrap()
                    | ((headers_frame.exclusive.unwrap() as u32) << 31))
                    .to_be_bytes(),
            );
            payload.push(headers_frame.weight.unwrap());
        }

        payload.extend(headers_frame.header_block_fragment);

        if headers_frame.pad_length > 0 {
            payload.extend(vec![0, headers_frame.pad_length]);
        }

        let mut header_bytes: Vec<u8> = headers_frame.header.into();
        header_bytes.extend(payload);
        header_bytes
    }
}

use std::io::Read;

use crate::{
    encode_to::EncodeTo,
    http2::{
        connection_state::ConnectionState,
        frames::frame::{FrameHeader, FrameType},
    },
    response::Response,
};

#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
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
        bits |= u8::from(flags.end_stream);
        bits |= u8::from(flags.end_headers) << 2;
        bits |= u8::from(flags.padded) << 3;
        bits |= u8::from(flags.priority) << 5;
        bits
    }
}

#[derive(Debug)]
pub struct HeadersFrame {
    pub header: FrameHeader<HeadersFrameFlags>,
    pad_length: u8,
    exclusive: Option<bool>,
    pub stream_dependency: Option<u32>, // 31 bits
    weight: Option<u8>,
    pub header_block_fragment: Vec<u8>,
}

impl TryFrom<&[u8]> for HeadersFrame {
    type Error = String;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let mut buf = buf;
        let header: FrameHeader<HeadersFrameFlags> = FrameHeader::try_from(buf)?;

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

impl From<(&Response, &mut ConnectionState<'_>)> for HeadersFrame {
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
            #[allow(clippy::cast_possible_truncation)]
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

impl EncodeTo for HeadersFrame {
    fn encode_to(self, buf: &mut Vec<u8>) {
        let priority = self.header.flags.priority;
        let padded = self.header.flags.padded;
        self.header.encode_to(buf);

        if padded {
            buf.push(self.pad_length);
        }

        if priority {
            buf.extend(
                (self.stream_dependency.unwrap() | (u32::from(self.exclusive.unwrap()) << 31))
                    .to_be_bytes(),
            );
            buf.push(self.weight.unwrap());
        }

        buf.extend(self.header_block_fragment);

        if self.pad_length > 0 {
            buf.extend(vec![0; self.pad_length as usize]);
        }
    }
}

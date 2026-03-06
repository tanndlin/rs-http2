use std::io::Read;

use crate::{
    encode_to::EncodeTo,
    http2::{
        error::HTTP2Error,
        frames::frame::{Frame, FrameHeader, FrameType},
    },
    response::Response,
};

#[derive(Debug)]
pub struct DataFrameFlags {
    pub padding: bool,
    pub end_stream: bool,
}

impl From<u8> for DataFrameFlags {
    fn from(bits: u8) -> Self {
        Self {
            padding: bits & 8 > 0,    // bit 3
            end_stream: bits & 1 > 0, // bit 0
        }
    }
}

impl From<DataFrameFlags> for u8 {
    fn from(flags: DataFrameFlags) -> Self {
        let mut bits = 0u8;
        if flags.padding {
            bits |= 8; // bit 3
        }
        if flags.end_stream {
            bits |= 1; // bit 0
        }
        bits
    }
}

#[derive(Debug)]
pub struct DataFrame {
    pub header: FrameHeader<DataFrameFlags>,
    pub pad_length: u8, // Exists if padding flag is set
    pub data: Vec<u8>,
}

impl DataFrame {
    #[allow(clippy::cast_possible_truncation)]
    pub fn new(stream_id: u32, data: Vec<u8>, end_stream: bool) -> Self {
        Self {
            header: FrameHeader {
                length: data.len() as u32,
                frame_type: FrameType::Data,
                flags: DataFrameFlags {
                    padding: false,
                    end_stream,
                },
                stream_id,
            },
            pad_length: 0,
            data,
        }
    }
}

impl TryFrom<&[u8]> for DataFrame {
    type Error = HTTP2Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let mut buf = buf;
        let header: FrameHeader<DataFrameFlags> = FrameHeader::try_from(buf)?;
        buf = &buf[9..];
        let pad_length = if header.flags.padding {
            let val = buf[0];
            buf = &buf[1..];
            val
        } else {
            0
        };

        let data_length = (header.length - u32::from(pad_length)) as usize;
        let mut data = vec![0u8; data_length];
        buf.read_exact(&mut data).unwrap();

        Ok(Self {
            header,
            pad_length,
            data,
        })
    }
}

impl From<Response> for DataFrame {
    fn from(res: Response) -> Self {
        Self {
            header: FrameHeader {
                #[allow(clippy::cast_possible_truncation)]
                length: res.body.len() as u32,
                frame_type: FrameType::Data,
                flags: DataFrameFlags {
                    padding: false,
                    end_stream: true,
                },
                stream_id: res.stream_id,
            },
            pad_length: 0,
            data: res.body,
        }
    }
}

impl From<DataFrame> for Frame {
    fn from(frame: DataFrame) -> Self {
        Frame::Data(frame)
    }
}

impl EncodeTo for DataFrame {
    fn encode_to(self, buf: &mut Vec<u8>) {
        let padding = self.header.flags.padding;
        self.header.encode_to(buf);

        if padding {
            buf.push(self.pad_length);
        }

        buf.extend(self.data);

        if self.pad_length > 0 {
            buf.extend(vec![0; self.pad_length as usize]);
        }
    }
}

impl Default for DataFrame {
    fn default() -> Self {
        DataFrame {
            header: FrameHeader {
                length: 0,
                frame_type: FrameType::Data,
                flags: DataFrameFlags {
                    padding: false,
                    end_stream: false,
                },
                stream_id: 0,
            },
            pad_length: 0,
            data: vec![],
        }
    }
}

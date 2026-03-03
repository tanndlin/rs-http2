use std::io::Read;

use crate::{
    http2::frames::{
        frame::{FrameHeader, FrameType},
        frame_trait::Frame,
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
    header: FrameHeader<DataFrameFlags>,
    pad_length: u8, // Exists if padding flag is set
    data: Vec<u8>,
}

impl Frame for DataFrame {
    fn get_length(&self) -> usize {
        self.header.length as usize
    }
}

impl TryFrom<&[u8]> for DataFrame {
    type Error = String;

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

        let data_length = (header.length - pad_length as u32) as usize;
        let mut data = vec![0u8; data_length];
        buf.read_exact(&mut data)
            .map_err(|_| format!("DataFrame buffer had less than {data_length} bytes"))?;

        Ok(Self {
            header,
            pad_length,
            data,
        })
    }
}

impl From<&Response> for DataFrame {
    fn from(res: &Response) -> Self {
        Self {
            header: FrameHeader {
                length: res.body.len() as u32,
                frame_type: FrameType::Data,
                flags: DataFrameFlags {
                    padding: false,
                    end_stream: true,
                },
                stream_id: res.stream_id,
            },
            pad_length: 0,
            data: res.body.clone(),
        }
    }
}

impl From<DataFrame> for Vec<u8> {
    fn from(data_frame: DataFrame) -> Self {
        let mut payload = vec![];
        if data_frame.header.flags.padding {
            payload.push(data_frame.pad_length)
        }

        payload.extend(data_frame.data);

        if data_frame.pad_length > 0 {
            payload.extend(vec![0; data_frame.pad_length as usize])
        }

        let mut header_bytes: Vec<u8> = data_frame.header.into();
        header_bytes.extend(payload);
        header_bytes
    }
}

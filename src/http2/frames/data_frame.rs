use std::io::Read;

use crate::http2::frames::{frame::FramePrefix, frame_trait::Frame};

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

#[derive(Debug)]
pub struct DataFrame {
    frame_prefix: FramePrefix<DataFrameFlags>,
    pad_length: u8, // Exists if padding flag is set
    data: Vec<u8>,
}

impl Frame for DataFrame {
    fn get_length(&self) -> usize {
        self.frame_prefix.length as usize
    }
}

impl TryFrom<&[u8]> for DataFrame {
    type Error = String;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let mut buf = buf;
        let frame_prefix: FramePrefix<DataFrameFlags> = FramePrefix::try_from(buf)?;
        buf = &buf[9..];
        let pad_length = if frame_prefix.flags.padding {
            let val = buf[0];
            buf = &buf[1..];
            val
        } else {
            0
        };

        let data_length = (frame_prefix.length - pad_length as u32) as usize;
        let mut data = vec![0u8; data_length];
        buf.read_exact(&mut data)
            .map_err(|_| format!("DataFrame buffer had less than {data_length} bytes"))?;

        Ok(Self {
            frame_prefix,
            pad_length,
            data,
        })
    }
}

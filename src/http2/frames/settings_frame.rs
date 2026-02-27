use crate::http2::frames::{
    frame::{FrameHeader, FrameType},
    frame_trait::Frame,
};

#[derive(Debug, Default)]
pub struct SettingsFrameFlags {
    pub ack: bool,
}

impl From<u8> for SettingsFrameFlags {
    fn from(bits: u8) -> Self {
        Self {
            ack: bits & 1 > 0, // bit 0
        }
    }
}

impl From<SettingsFrameFlags> for u8 {
    fn from(flags: SettingsFrameFlags) -> Self {
        let mut bits = 0u8;

        bits |= flags.ack as u8; // bit 0

        bits
    }
}

#[derive(Debug, Default)]
pub struct SettingsFrame {
    pub header: FrameHeader<SettingsFrameFlags>,
    header_table_size: Option<u32>,
    enable_push: Option<bool>,
    max_concurrent_streams: Option<u32>,
    initial_window_size: Option<u32>,
    max_frame_size: Option<u32>,
    max_header_list_size: Option<u32>,
}

impl SettingsFrame {
    pub fn new_ack(stream_identifier: u32) -> Self {
        Self {
            header: FrameHeader {
                length: 0,
                frame_type: FrameType::Settings,
                flags: SettingsFrameFlags { ack: true },
                stream_identifier,
            },
            ..Default::default()
        }
    }
}

impl TryFrom<&[u8]> for SettingsFrame {
    type Error = String;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let header = FrameHeader::try_from(buf)?;
        let length = header.length as usize;
        let buf = &buf[9..];
        if buf.len() < length {
            return Err(format!(
                "Settings frame length does not match the length in the frame header. {} out of {length} bytes left",
                buf.len()
            ));
        }

        if !length.is_multiple_of(6) {
            return Err("Settins frame length must be a multiple of 6".to_string());
        }

        let mut ret = Self {
            header,
            ..Default::default()
        };
        let mut offset = 0;
        while offset < length {
            let ident = u16::from_be_bytes(buf[offset..offset + 2].try_into().unwrap());
            let value = u32::from_be_bytes(buf[offset + 2..offset + 6].try_into().unwrap());

            match ident {
                1 => ret.header_table_size = Some(value),
                2 => ret.enable_push = Some(value != 0),
                3 => ret.max_concurrent_streams = Some(value),
                4 => ret.initial_window_size = Some(value),
                5 => ret.max_frame_size = Some(value),
                6 => ret.max_header_list_size = Some(value),
                _ => (),
            }

            offset += 6;
        }

        Ok(ret)
    }
}

impl From<SettingsFrame> for Vec<u8> {
    fn from(frame: SettingsFrame) -> Self {
        let mut ret: Vec<u8> = frame.header.into();

        if let Some(val) = frame.header_table_size {
            ret.push(1);
            ret.extend(val.to_be_bytes());
        }

        if let Some(val) = frame.enable_push {
            ret.push(2);
            ret.push(val as u8);
        }

        if let Some(val) = frame.max_concurrent_streams {
            ret.push(3);
            ret.extend(val.to_be_bytes());
        }

        if let Some(val) = frame.initial_window_size {
            ret.push(4);
            ret.extend(val.to_be_bytes());
        }

        if let Some(val) = frame.max_frame_size {
            ret.push(5);
            ret.extend(val.to_be_bytes());
        }

        if let Some(val) = frame.max_header_list_size {
            ret.push(6);
            ret.extend(val.to_be_bytes());
        }

        ret
    }
}

impl Frame for SettingsFrame {
    fn get_length(&self) -> usize {
        9 + self.header.length as usize
    }
}

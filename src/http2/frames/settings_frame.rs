use crate::http2::frames::{frame::FramePrefix, frame_trait::Frame};

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

#[derive(Debug, Default)]
pub struct SettingsFrame {
    frame_prefix: FramePrefix<SettingsFrameFlags>,
    header_table_size: Option<u32>,
    enable_push: Option<bool>,
    max_concurrent_streams: Option<u32>,
    initial_window_size: Option<u32>,
    max_frame_size: Option<u32>,
    max_header_list_size: Option<u32>,
}

impl TryFrom<&[u8]> for SettingsFrame {
    type Error = String;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        let frame_prefix = FramePrefix::try_from(buf)?;
        let length = frame_prefix.length as usize;
        let buf = &buf[9..];
        if buf.len() < length {
            return Err(format!(
                "Settings frame length does not match the length in the frame prefix. {} out of {length} bytes left",
                buf.len()
            ));
        }

        if !length.is_multiple_of(6) {
            return Err("Settins frame length must be a multiple of 6".to_string());
        }

        let mut ret = Self {
            frame_prefix,
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

impl Frame for SettingsFrame {
    fn get_length(&self) -> usize {
        9 + self.frame_prefix.length as usize
    }
}

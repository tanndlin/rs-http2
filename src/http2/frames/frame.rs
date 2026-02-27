use crate::http2::frames::{data_frame::DataFrame, frame_trait, settings_frame::SettingsFrame};

#[repr(u8)]
#[derive(Debug, Default)]
pub enum FrameType {
    #[default]
    Data = 0,
    Settings = 4,
}

impl From<u8> for FrameType {
    fn from(value: u8) -> Self {
        match value {
            0 => FrameType::Data,
            4 => FrameType::Settings,
            _ => FrameType::Data, // Default case for unknown frame types
        }
    }
}

#[derive(Debug)]
pub enum Frame {
    DataFrame(DataFrame),
    SettingsFrame(SettingsFrame),
}

impl frame_trait::Frame for Frame {
    fn get_length(&self) -> usize {
        match self {
            Frame::DataFrame(f) => f.get_length(),
            Frame::SettingsFrame(f) => f.get_length(),
        }
    }
}

impl TryFrom<&[u8]> for Frame {
    type Error = String;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        if buf.len() < 9 {
            return Err(
                "Tried to parse frame but buffer was less than 9 bytes for frame header"
                    .to_string(),
            );
        }

        let frame_type = FrameType::from(buf[3]);
        Ok(match frame_type {
            FrameType::Data => Frame::DataFrame(DataFrame::try_from(buf)?),
            FrameType::Settings => Frame::SettingsFrame(SettingsFrame::try_from(buf)?),
        })
    }
}

#[derive(Debug, Default)]
pub struct FrameHeader<T>
where
    T: From<u8>,
{
    pub length: u32,           //24 bits
    pub frame_type: FrameType, // 8 bits
    pub flags: T,
    pub stream_identifier: u32, // 31 bits (R infront)
}

impl<T> From<FrameHeader<T>> for Vec<u8>
where
    T: From<u8>,
    T: Into<u8>,
{
    fn from(val: FrameHeader<T>) -> Self {
        let mut buf = vec![
            (val.length >> 16) as u8,
            (val.length >> 8) as u8,
            val.length as u8,
            val.frame_type as u8,
            val.flags.into(),
        ];
        buf.extend_from_slice(&val.stream_identifier.to_be_bytes());
        buf
    }
}

impl<T> TryFrom<&[u8]> for FrameHeader<T>
where
    T: From<u8>,
{
    type Error = String;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        if buf.len() < 9 {
            return Err("Frame header must be at least 9 bytes".to_string());
        }

        let length = ((buf[0] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[2] as u32);
        let frame_type = FrameType::from(buf[3]);
        let flag_bits = buf[4];
        let flags = T::from(flag_bits);
        let stream_identifier = u32::from_be_bytes([buf[5], buf[6], buf[7], buf[8]]) & !(0b1 << 31);

        Ok(Self {
            length,
            frame_type,
            flags,
            stream_identifier,
        })
    }
}

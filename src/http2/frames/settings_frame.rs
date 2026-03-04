use crate::http2::frames::frame::{FrameHeader, FrameType};

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

#[repr(u16)]
pub enum SettingsIdentifier {
    HeaderTableSize = 1,
    EnablePush = 2,
    MaxConcurrentStreams = 3,
    InitialWindowSize = 4,
    MaxFrameSize = 5,
    MaxHeaderListSize = 6,
}

impl TryFrom<u16> for SettingsIdentifier {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::HeaderTableSize),
            2 => Ok(Self::EnablePush),
            3 => Ok(Self::MaxConcurrentStreams),
            4 => Ok(Self::InitialWindowSize),
            5 => Ok(Self::MaxFrameSize),
            6 => Ok(Self::MaxHeaderListSize),
            _ => Err(format!("Invalid settings identifier: {value}")),
        }
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
                stream_id: stream_identifier,
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
            let ident = u16::from_be_bytes(
                buf[offset..offset + 2]
                    .try_into()
                    .map_err(|_| "Invalid data length")?,
            );
            let ident = SettingsIdentifier::try_from(ident)?;
            let value = u32::from_be_bytes(buf[offset + 2..offset + 6].try_into().unwrap());

            match ident {
                SettingsIdentifier::HeaderTableSize => ret.header_table_size = Some(value),
                SettingsIdentifier::EnablePush => ret.enable_push = Some(value > 0),
                SettingsIdentifier::MaxConcurrentStreams => {
                    ret.max_concurrent_streams = Some(value)
                }
                SettingsIdentifier::InitialWindowSize => ret.initial_window_size = Some(value),
                SettingsIdentifier::MaxFrameSize => ret.max_frame_size = Some(value),
                SettingsIdentifier::MaxHeaderListSize => ret.max_header_list_size = Some(value),
            }

            offset += 6;
        }

        Ok(ret)
    }
}

impl From<SettingsFrame> for Vec<u8> {
    fn from(frame: SettingsFrame) -> Self {
        let mut ret: Vec<u8> = frame.header.into();

        if let Some(size) = frame.header_table_size {
            ret.extend_from_slice(&(SettingsIdentifier::HeaderTableSize as u16).to_be_bytes());
            ret.extend_from_slice(&size.to_be_bytes());
        }
        if let Some(enable) = frame.enable_push {
            ret.extend_from_slice(&(SettingsIdentifier::EnablePush as u16).to_be_bytes());
            ret.extend_from_slice(&(enable as u32).to_be_bytes());
        }
        if let Some(max) = frame.max_concurrent_streams {
            ret.extend_from_slice(&(SettingsIdentifier::MaxConcurrentStreams as u16).to_be_bytes());
            ret.extend_from_slice(&max.to_be_bytes());
        }
        if let Some(size) = frame.initial_window_size {
            ret.extend_from_slice(&(SettingsIdentifier::InitialWindowSize as u16).to_be_bytes());
            ret.extend_from_slice(&size.to_be_bytes());
        }
        if let Some(size) = frame.max_frame_size {
            ret.extend_from_slice(&(SettingsIdentifier::MaxFrameSize as u16).to_be_bytes());
            ret.extend_from_slice(&size.to_be_bytes());
        }
        if let Some(size) = frame.max_header_list_size {
            ret.extend_from_slice(&(SettingsIdentifier::MaxHeaderListSize as u16).to_be_bytes());
            ret.extend_from_slice(&size.to_be_bytes());
        }

        ret
    }
}

pub struct SettingsFrameBuilder {
    header: Option<FrameHeader<SettingsFrameFlags>>,
    header_table_size: Option<u32>,
    enable_push: Option<bool>,
    max_concurrent_streams: Option<u32>,
    initial_window_size: Option<u32>,
    max_frame_size: Option<u32>,
    max_header_list_size: Option<u32>,
}

impl SettingsFrameBuilder {
    pub fn new() -> Self {
        SettingsFrameBuilder {
            header: None,
            header_table_size: None,
            enable_push: None,
            max_concurrent_streams: None,
            initial_window_size: None,
            max_frame_size: None,
            max_header_list_size: None,
        }
    }

    pub fn header_table_size(mut self, size: u32) -> Self {
        self.header_table_size = Some(size);
        self
    }

    pub fn enable_push(mut self, enable: bool) -> Self {
        self.enable_push = Some(enable);
        self
    }

    pub fn max_concurrent_streams(mut self, max: u32) -> Self {
        self.max_concurrent_streams = Some(max);
        self
    }

    pub fn initial_window_size(mut self, size: u32) -> Self {
        self.initial_window_size = Some(size);
        self
    }

    pub fn max_frame_size(mut self, size: u32) -> Self {
        self.max_frame_size = Some(size);
        self
    }

    pub fn max_header_list_size(mut self, size: u32) -> Self {
        self.max_header_list_size = Some(size);
        self
    }

    pub fn build(self) -> SettingsFrame {
        let length = self.calc_length();
        let SettingsFrameBuilder {
            header,
            header_table_size,
            enable_push,
            max_concurrent_streams,
            initial_window_size,
            max_frame_size,
            max_header_list_size,
        } = self;

        let header = match header {
            Some(mut h) => {
                h.length = length;
                h
            }
            None => FrameHeader::<SettingsFrameFlags> {
                length,
                frame_type: FrameType::Settings,
                stream_id: 0,
                flags: SettingsFrameFlags { ack: false },
            },
        };

        SettingsFrame {
            header,
            header_table_size,
            enable_push,
            max_concurrent_streams,
            initial_window_size,
            max_frame_size,
            max_header_list_size,
        }
    }

    fn calc_length(&self) -> u32 {
        let mut specified_parameters = 0u32;

        if self.header_table_size.is_some() {
            specified_parameters += 1;
        }
        if self.enable_push.is_some() {
            specified_parameters += 1;
        }
        if self.max_concurrent_streams.is_some() {
            specified_parameters += 1;
        }
        if self.initial_window_size.is_some() {
            specified_parameters += 1;
        }
        if self.max_frame_size.is_some() {
            specified_parameters += 1;
        }
        if self.max_header_list_size.is_some() {
            specified_parameters += 1;
        }

        specified_parameters * 6
    }
}

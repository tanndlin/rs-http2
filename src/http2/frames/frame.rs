use crate::{
    encode_to::EncodeTo,
    http2::{
        error::{HTTP2Error, HTTP2ErrorCode},
        frames::{
            continuation_frame::ContinuationFrame, data_frame::DataFrame,
            go_away_frame::GoAwayFrame, headers_frame::HeadersFrame, ping_frame::PingFrame,
            priority_frame::PriorityFrame, push_promise_frame::PushPromiseFrame,
            rst_frame::RstFrame, settings_frame::SettingsFrame,
            window_update_frame::WindowUpdateFrame,
        },
    },
};

#[repr(u8)]
#[derive(Debug, Default)]
pub enum FrameType {
    #[default]
    Data = 0,
    Headers = 1,
    Priority = 2,
    RstStream = 3,
    Settings = 4,
    PushPromise = 5,
    Ping = 6,
    GoAway = 7,
    WindowUpdate = 8,
    Continuation = 9,
}

impl TryFrom<u8> for FrameType {
    type Error = HTTP2Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Data),
            1 => Ok(Self::Headers),
            2 => Ok(Self::Priority),
            3 => Ok(Self::RstStream),
            4 => Ok(Self::Settings),
            5 => Ok(Self::PushPromise),
            6 => Ok(Self::Ping),
            7 => Ok(Self::GoAway),
            8 => Ok(Self::WindowUpdate),
            9 => Ok(Self::Continuation),
            _ => Err(HTTP2Error::Connection(HTTP2ErrorCode::NoError)), // Discard
        }
    }
}

#[derive(Debug)]
pub enum Frame {
    Data(DataFrame),
    Headers(HeadersFrame),
    Priority(PriorityFrame),
    RstStream(RstFrame),
    Settings(SettingsFrame),
    PushPromise(PushPromiseFrame),
    Ping(PingFrame),
    GoAway(GoAwayFrame),
    WindowUpdate(WindowUpdateFrame),
    Continuation(ContinuationFrame),
}

impl TryFrom<&[u8]> for Frame {
    type Error = HTTP2Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        assert!(
            buf.len() >= 9,
            "Tried to parse frame but buffer was less than 9 bytes for frame header"
        );

        let frame_type = FrameType::try_from(buf[3])
            .map_err(|_| HTTP2Error::Connection(HTTP2ErrorCode::NoError))?; // Discard
        Ok(match frame_type {
            FrameType::Data => Frame::Data(DataFrame::try_from(buf)?),
            FrameType::Headers => Frame::Headers(HeadersFrame::try_from(buf)?),
            FrameType::Priority => Frame::Priority(PriorityFrame::try_from(buf)?),
            FrameType::RstStream => Frame::RstStream(RstFrame::try_from(buf)?),
            FrameType::Settings => Frame::Settings(SettingsFrame::try_from(buf)?),
            FrameType::PushPromise => Frame::PushPromise(PushPromiseFrame::try_from(buf)?),
            FrameType::Ping => Frame::Ping(PingFrame::try_from(buf)?),
            FrameType::GoAway => Frame::GoAway(GoAwayFrame::try_from(buf)?),
            FrameType::WindowUpdate => Frame::WindowUpdate(WindowUpdateFrame::try_from(buf)?),
            FrameType::Continuation => Frame::Continuation(ContinuationFrame::try_from(buf)?),
        })
    }
}

impl Frame {
    pub fn get_stream_id(&self) -> u32 {
        match self {
            Frame::Data(f) => f.header.stream_id,
            Frame::Headers(f) => f.header.stream_id,
            Frame::Priority(f) => f.header.stream_id,
            Frame::RstStream(f) => f.header.stream_id,
            Frame::Settings(f) => f.header.stream_id,
            Frame::PushPromise(f) => f.header.stream_id,
            Frame::Ping(f) => f.header.stream_id,
            Frame::GoAway(f) => f.header.stream_id,
            Frame::WindowUpdate(f) => f.header.stream_id,
            Frame::Continuation(f) => f.header.stream_id,
        }
    }

    pub fn to_bytes(self) -> Vec<u8> {
        match self {
            Frame::Data(f) => f.to_bytes(),
            Frame::Headers(f) => f.to_bytes(),
            Frame::Priority(f) => f.to_bytes(),
            Frame::RstStream(f) => f.to_bytes(),
            Frame::Settings(f) => f.to_bytes(),
            Frame::PushPromise(f) => f.to_bytes(),
            Frame::Ping(f) => f.to_bytes(),
            Frame::GoAway(f) => f.to_bytes(),
            Frame::WindowUpdate(f) => f.to_bytes(),
            Frame::Continuation(f) => f.to_bytes(),
        }
    }
}

impl EncodeTo for Frame {
    fn encode_to(self, buf: &mut Vec<u8>) {
        match self {
            Frame::Data(f) => f.encode_to(buf),
            Frame::Headers(f) => f.encode_to(buf),
            Frame::Priority(f) => f.encode_to(buf),
            Frame::RstStream(f) => f.encode_to(buf),
            Frame::Settings(f) => f.encode_to(buf),
            Frame::PushPromise(f) => f.encode_to(buf),
            Frame::Ping(f) => f.encode_to(buf),
            Frame::GoAway(f) => f.encode_to(buf),
            Frame::WindowUpdate(f) => f.encode_to(buf),
            Frame::Continuation(f) => f.encode_to(buf),
        }
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
    pub stream_id: u32, // 31 bits (R infront)
}

impl<T> EncodeTo for FrameHeader<T>
where
    T: From<u8>,
    T: Into<u8>,
{
    #[allow(clippy::cast_possible_truncation)]
    fn encode_to(self, buf: &mut Vec<u8>) {
        buf.push((self.length >> 16) as u8);
        buf.push((self.length >> 8) as u8);
        buf.push(self.length as u8);
        buf.push(self.frame_type as u8);
        buf.push(self.flags.into());
        buf.extend(self.stream_id.to_be_bytes());
    }
}

impl<T> TryFrom<&[u8]> for FrameHeader<T>
where
    T: From<u8>,
{
    type Error = HTTP2Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        assert!(buf.len() >= 9, "Frame header must be at least 9 bytes");

        let length = (u32::from(buf[0]) << 16) | (u32::from(buf[1]) << 8) | u32::from(buf[2]);
        let frame_type = FrameType::try_from(buf[3])?;
        let flag_bits = buf[4];
        let flags = T::from(flag_bits);
        let stream_identifier = u32::from_be_bytes([buf[5], buf[6], buf[7], buf[8]]) & !(0b1 << 31);

        Ok(Self {
            length,
            frame_type,
            flags,
            stream_id: stream_identifier,
        })
    }
}

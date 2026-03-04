#[derive(Debug)]
pub enum HTTP2Error {
    Connection(HTTP2ErrorCode),
    Stream(StreamError),
}

#[allow(dead_code)]
#[derive(Debug)]
#[repr(u32)]
pub enum HTTP2ErrorCode {
    NoError = 0,
    ProtocolError = 1,
    InternalError = 2,
    FlowControlError = 3,
    SettingsTimeout = 4,
    StreamClosed = 5,
    FrameSizeError = 6,
    RefusedStream = 7,
    Cancel = 8,
    CompressionError = 9,
    ConnectError = 10,
    EnhanceYourCalm = 11,
    InadequateSecurity = 12,
    HTTP11Required = 13,
}

#[derive(Debug)]
pub struct StreamError {
    pub stream_id: u32,
    pub error_code: HTTP2ErrorCode,
}

impl StreamError {
    pub fn new(stream_id: u32, error_code: HTTP2ErrorCode) -> Self {
        Self {
            stream_id,
            error_code,
        }
    }
}

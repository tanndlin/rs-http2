#[derive(Debug)]
pub struct SettingsFrame {
    header_table_size: usize,
    enable_push: bool,
    max_concurrent_streams: usize,
    initial_window_size: usize,
    max_frame_size: usize,
    max_header_list_size: usize,
}

impl SettingsFrame {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 6 {
            return Err("Settings frame must be at least 6 bytes".to_string());
        }

        let header_table_size = u32::from_be_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let enable_push = bytes[4] != 0;
        let max_concurrent_streams = u32::from_be_bytes(bytes[5..9].try_into().unwrap()) as usize;
        let initial_window_size = u32::from_be_bytes(bytes[9..13].try_into().unwrap()) as usize;
        let max_frame_size = u32::from_be_bytes(bytes[13..17].try_into().unwrap()) as usize;
        let max_header_list_size = u32::from_be_bytes(bytes[17..21].try_into().unwrap()) as usize;

        Ok(Self {
            header_table_size,
            enable_push,
            max_concurrent_streams,
            initial_window_size,
            max_frame_size,
            max_header_list_size,
        })
    }
}

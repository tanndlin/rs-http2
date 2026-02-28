pub fn u32_from_3_bytes(buf: &[u8; 3]) -> u32 {
    (buf[0] as u32) << 16 | (buf[1] as u32) << 8 | (buf[2] as u32)
}

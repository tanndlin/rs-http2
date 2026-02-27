use hpack::{Decoder, Encoder};

pub struct ConnectionState<'a> {
    pub decoder: Decoder<'a>,
    pub encoder: Encoder<'a>,
}

impl<'a> ConnectionState<'a> {
    pub fn new() -> Self {
        ConnectionState {
            decoder: Decoder::new(),
            encoder: Encoder::new(),
        }
    }
}

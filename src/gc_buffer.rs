use std::{io::Read, net::TcpStream};

use openssl::ssl::SslStream;

pub struct GCBuffer {
    data: Vec<u8>,
    cursor: usize,
    read_buffer: [u8; 4096],
}

impl GCBuffer {
    pub fn new() -> Self {
        Self {
            data: vec![],
            cursor: 0,
            read_buffer: [0; 4096],
        }
    }

    pub fn read_from_stream(
        &mut self,
        stream: &mut SslStream<TcpStream>,
    ) -> Result<usize, std::io::Error> {
        let bytes_read = stream.read(&mut self.read_buffer)?;
        self.data.extend_from_slice(&self.read_buffer[..bytes_read]);
        Ok(bytes_read)
    }

    pub fn peek(&self, n: usize) -> &[u8] {
        &self.data[self.cursor..self.cursor + n]
    }

    pub fn read_n_bytes(&mut self, n: usize) -> Vec<u8> {
        let ret = self.data[self.cursor..self.cursor + n].to_vec();
        self.cursor += n;
        self.compress();

        ret
    }

    pub fn len(&self) -> usize {
        self.data.len() - self.cursor
    }

    fn compress(&mut self) {
        if self.cursor > 32768 {
            self.data.drain(0..self.cursor);
            self.cursor = 0;
        }
    }
}

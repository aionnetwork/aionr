pub struct NativeEncoder {
    buffer: Vec<u8>,
}

impl NativeEncoder {
    pub fn new() -> NativeEncoder {
        let buffer: Vec<u8> = Vec::new();

        NativeEncoder {
            buffer: buffer,
        }
    }

    pub fn encode_byte(&mut self, n: u8) { self.buffer.push(n); }

    pub fn encode_short(&mut self, n: u16) {
        self.buffer.push((n >> 8) as u8);
        self.buffer.push(n as u8);
    }

    pub fn encode_int(&mut self, n: u32) {
        self.buffer.push((n >> 24) as u8);
        self.buffer.push((n >> 16) as u8);
        self.buffer.push((n >> 8) as u8);
        self.buffer.push(n as u8);
    }

    pub fn encode_long(&mut self, n: u64) {
        self.buffer.push((n >> 56) as u8);
        self.buffer.push((n >> 48) as u8);
        self.buffer.push((n >> 40) as u8);
        self.buffer.push((n >> 32) as u8);
        self.buffer.push((n >> 24) as u8);
        self.buffer.push((n >> 16) as u8);
        self.buffer.push((n >> 8) as u8);
        self.buffer.push(n as u8);
    }

    pub fn encode_bytes(&mut self, bytes: &Vec<u8>) {
        self.encode_int(bytes.len() as u32);
        self.buffer.append(&mut bytes.clone());
    }

    pub fn to_bytes(&self) -> Vec<u8> { self.buffer.clone() }
}

pub struct NativeDecoder {
    bytes: Vec<u8>,
    index: usize,
}

impl NativeDecoder {
    pub fn new(bytes: &Vec<u8>) -> NativeDecoder {
        NativeDecoder {
            bytes: bytes.clone(),
            index: 0,
        }
    }

    pub fn decode_byte(&mut self) -> Result<u8, &'static str> {
        match self.require(1) {
            true => {
                let ret = self.bytes[self.index];
                self.index = self.index + 1;
                Ok(ret)
            }
            false => Err("Index out of bounds"),
        }
    }

    pub fn decode_short(&mut self) -> Result<u16, &'static str> {
        match self.require(2) {
            true => {
                let ret =
                    ((self.bytes[self.index] as u16) << 8) | (self.bytes[self.index + 1] as u16);
                self.index = self.index + 2;
                Ok(ret)
            }
            false => Err("Index out of bounds"),
        }
    }

    pub fn decode_int(&mut self) -> Result<u32, &'static str> {
        match self.require(4) {
            true => {
                let ret = ((self.bytes[self.index] as u32) << 24)
                    | ((self.bytes[self.index + 1] as u32) << 16)
                    | ((self.bytes[self.index + 2] as u32) << 8)
                    | (self.bytes[self.index + 3] as u32);
                self.index = self.index + 4;
                Ok(ret)
            }
            false => Err("Index out of bounds"),
        }
    }

    pub fn decode_long(&mut self) -> Result<u64, &'static str> {
        match self.require(8) {
            true => {
                let ret = ((self.bytes[self.index] as u64) << 56)
                    | ((self.bytes[self.index + 1] as u64) << 48)
                    | ((self.bytes[self.index + 2] as u64) << 40)
                    | ((self.bytes[self.index + 3] as u64) << 32)
                    | ((self.bytes[self.index + 4] as u64) << 24)
                    | ((self.bytes[self.index + 5] as u64) << 16)
                    | ((self.bytes[self.index + 6] as u64) << 8)
                    | (self.bytes[self.index + 7] as u64);
                self.index = self.index + 8;
                Ok(ret)
            }
            false => Err("Index out of bounds"),
        }
    }

    pub fn decode_bytes(&mut self) -> Result<Vec<u8>, &'static str> {
        let size = self.decode_int()? as usize;
        match self.require(size) {
            true => {
                let slice = self.bytes.as_slice();
                let ret = slice[self.index..self.index + size].to_vec();
                self.index = self.index + size;
                Ok(ret)
            }
            false => {
                Err("Index out of bounds")
            },
        }
    }

    pub fn require(&self, n: usize) -> bool { self.bytes.len() - self.index >= n }
}

#[cfg(test)]
mod test {
    use self::super::{NativeDecoder, NativeEncoder};
    use std::{u16, u32, u64, u8};

    #[test]
    pub fn test_codec() {
        let mut encoder = NativeEncoder::new();
        encoder.encode_byte(u8::MIN);
        encoder.encode_byte(u8::MAX);
        encoder.encode_short(u16::MIN);
        encoder.encode_short(u16::MAX);
        encoder.encode_int(u32::MIN);
        encoder.encode_int(u32::MAX);
        encoder.encode_long(u64::MIN);
        encoder.encode_long(u64::MAX);
        encoder.encode_bytes(&"".as_bytes().to_vec());
        encoder.encode_bytes(&"test".as_bytes().to_vec());
        let bytes = encoder.to_bytes();

        let mut decoder = NativeDecoder::new(&bytes);
        assert_eq!(u8::MIN, decoder.decode_byte().unwrap());
        assert_eq!(u8::MAX, decoder.decode_byte().unwrap());
        assert_eq!(u16::MIN, decoder.decode_short().unwrap());
        assert_eq!(u16::MAX, decoder.decode_short().unwrap());
        assert_eq!(u32::MIN, decoder.decode_int().unwrap());
        assert_eq!(u32::MAX, decoder.decode_int().unwrap());
        assert_eq!(u64::MIN, decoder.decode_long().unwrap());
        assert_eq!(u64::MAX, decoder.decode_long().unwrap());
        assert_eq!("".as_bytes().to_vec(), decoder.decode_bytes().unwrap());
        assert_eq!("test".as_bytes().to_vec(), decoder.decode_bytes().unwrap());
    }
}

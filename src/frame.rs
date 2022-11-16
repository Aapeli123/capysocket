use crate::ws_stream::{BYTES, TEXT, PONG};

pub struct FrameHeader {
    pub fin: bool,
    pub content_len: u64,
    pub is_masked: bool,
    pub opcode: u8,
    pub mask: Option<[u8; 4]>,
    pub header_len: u64,
}

impl FrameHeader {
    pub fn new(buffer: &[u8]) -> FrameHeader {
        let head = buffer[0];
        let fin = (head & 0b10000000) != 0;
        let opcode = head & 0b00001111;
        let content_len_and_mask_header = buffer[1];
        let is_masked = (content_len_and_mask_header & 0b10000000) != 0;
        let content_len_data = content_len_and_mask_header & 0b01111111;
        let content_len_len = if content_len_data == 126 {2} else if content_len_data == 127 {8} else {0};

        let mut content_len = 0;
        if content_len_len == 0 {
            content_len = content_len_data as u64;
        }
        
        for i in 0..content_len_len {
            let num = ((buffer[2 + i]) as u64) << ((content_len_len - 1 - i) * 8);
            content_len += num;
        };

        let mut mask = [0; 4];
        for i in 0..4 {
            let k = buffer[2 + content_len_len + i];
            mask[i] = k;
        };
        let header_len: u64 = 2 + content_len_len as u64 + 4;

        FrameHeader{
            fin,
            content_len,
            mask: Some(mask),
            is_masked,
            opcode,
            header_len
        }
    }

    fn is_control(&self) -> bool {
        self.opcode >= 0x8 && self.opcode <= 0xA
    }

    pub fn calculate_header_len(content_len: u64, is_masked: bool) -> u64 {
        let mut content_len_len = 0;
        if 126 <= content_len && content_len <= 65536 {
            content_len_len = 2;
        } else if content_len > 65536  {
            content_len_len = 8;
        }
        let mut mask_add = 0;
        if is_masked {
            mask_add = 4;
        }
        2 + content_len_len + mask_add
    }

    pub fn get_header_bytes(&self) -> Vec<u8> {
        let mut content_len_len = 0;
        let mut content_len_bit = self.content_len;
        if 126 <= self.content_len && self.content_len <= 65536 {
            content_len_len = 2;
            content_len_bit = 126;
        } else if self.content_len > 65536  {
            content_len_bit = 127;
            content_len_len = 8;
        }
        let content_len_bit = content_len_bit as u8;
        let mut content_len_bytes = Vec::new();
        for i in 0..content_len_len {
            let b = ((self.content_len >> ((content_len_len - i - 1) * 8)) &  255) as u8;
            content_len_bytes.push(b);
        }

        let mut head: u8 = 0;
        if self.fin {
            head |= 0b10000000
        }
        head |= self.opcode & 0b00001111;
        let mut second_bit: u8 = 0;
        if self.is_masked {
            second_bit |= 0b10000000;
        }
        second_bit |= content_len_bit;
        let mut header_bytes = vec![head, second_bit];
        header_bytes.append(&mut content_len_bytes);
        if self.is_masked {
            header_bytes.extend(&self.mask.unwrap());
        }
        header_bytes
    }
}


pub struct Frame {
    pub header: FrameHeader,
    pub contents: Vec<u8>
}

impl Frame {
    pub fn new(header: FrameHeader, mut contents: Vec<u8>) -> Frame {
        if header.is_masked {
            let mask = header.mask.unwrap(); 
            for i in 0..contents.len() {
                contents[i] = contents[i] ^ mask[i % 4];
            };
        }
        Frame {header, contents}
    }

    pub fn is_control(&self) -> bool {
        self.header.is_control()
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Frame {
        let header = FrameHeader{
            fin: true, 
            content_len: bytes.len() as u64,
            is_masked: false,
            mask: None,
            header_len: FrameHeader::calculate_header_len(bytes.len() as u64, false),
            opcode: BYTES,
        };
        Frame { header: header, contents: bytes }
    }

    pub fn from_text(text: &str) -> Frame {
        let bytes = text.as_bytes();
        let header = FrameHeader{
            fin: true, 
            content_len: bytes.len() as u64,
            is_masked: false,
            mask: None,
            header_len: FrameHeader::calculate_header_len(bytes.len() as u64, false),
            opcode: TEXT,
        };
        Frame { header: header, contents: bytes.to_vec() }
    }
    pub fn pong(ping_content: Vec<u8>) -> Frame {
        let header = FrameHeader{
            fin: true, 
            content_len: ping_content.len() as u64,
            is_masked: false,
            mask: None,
            header_len: FrameHeader::calculate_header_len(ping_content.len() as u64, false),
            opcode: PONG,
        };
        Frame { header: header, contents: ping_content }
    }
    pub fn to_bytes(&mut self) -> Vec<u8> {
        let mut bytes = Vec::<u8>::new();
        let mut header_bytes = self.header.get_header_bytes();
        bytes.append(&mut header_bytes);
        bytes.append(&mut self.contents);
        bytes
    } 
}

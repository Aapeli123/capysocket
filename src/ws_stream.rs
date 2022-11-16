use std::io::{Read, Write};
use httparse::{Request};
use sha1::{Sha1, Digest};
use base64;
use std::vec;
use crate::message::{Message};
use crate::frame::{Frame, FrameHeader};


const CONTINUE: u8 = 0x0;

pub const TEXT: u8 = 0x1;
pub const BYTES: u8 = 0x2;

pub const CLOSE: u8 = 0x8;
pub const PING: u8 = 0x9;
pub const PONG: u8 = 0xA;

const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
#[derive(Debug)]
pub struct WSStream<S: Read + Write> {
    tcp: S,
}

fn get_ws_accept(ws_key: String) -> String {
    let mut hasher = Sha1::new();
    let concated = ws_key + WS_GUID;
    hasher.update(concated);
    let hash = hasher.finalize();
    base64::encode(hash)
}




impl<S: Read + Write> WSStream<S> {
    pub fn from(mut stream: S) -> Result<WSStream<S>, String> {
        // Upgrades the connection in stream to websocket if it has sufficient headers.
        let mut data: [u8; 1024] = [0; 1024];
        let mut r = stream.read(&mut data);
        while r.is_err() {
            r = stream.read(&mut data);
        }
        let mut headers = [httparse::EMPTY_HEADER; 1024];
        {
            let mut req = Request::new(&mut headers);
            req.parse(&data).ok();
        }
        let mut key= String::new();
        let mut wants_to_upgrade = false;
        let mut key_found = false;
        for &h in headers.iter() {
            if h.name == "Sec-WebSocket-Key" {
                key_found = true;
                key = String::from_utf8_lossy(h.value).to_string();
            }
            if h.name == "Upgrade" {
                if String::from_utf8_lossy(h.value).contains("websocket") {
                    wants_to_upgrade = true;
                }
            }

            if key_found && wants_to_upgrade {
                break;
            }
        }

        if !wants_to_upgrade {
            let mut response = String::new();
            response.push_str("HTTP/1.1 200 OK\r\n");
            response.push_str("Connection: close\r\n");
            response.push_str("Content-Length: 0\r\n");
            response.push_str("Access-Control-Allow-Origin: *\r\n\r\n");
    
            stream.write(response.as_bytes()).unwrap();
            stream.flush().ok();
            return Err(String::from("Connection did not want to upgrade"));
        }

        let mut response = String::new();
        response.push_str("HTTP/1.1 101 Switching Protocols\r\n");
        response.push_str("Upgrade: websocket\r\n");
        response.push_str("Connection: Upgrade\r\n");
        response.push_str("Access-Control-Allow-Origin: *\r\n");
        response.push_str(format!("Sec-WebSocket-Accept: {}\r\n\r\n", get_ws_accept(key)).as_str());

        stream.write(response.as_bytes()).unwrap();
        stream.flush().ok();
        Ok(WSStream{
            tcp: stream,
        })
        
        // std::thread::sleep(time::Duration::from_secs(10));
    }

    pub fn write_text(&mut self, text: &str) -> Result<(), std::io::Error> {
        let mut dataframe = Frame::from_text(text);

        let frame_bytes = dataframe.to_bytes();
        
        self.tcp.write_all(&frame_bytes)
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), std::io::Error> {
        let mut dataframe = Frame::from_bytes(bytes.to_vec());

        let frame_bytes = dataframe.to_bytes();
        
        self.tcp.write_all(&frame_bytes)
    }

    pub fn read_message(&mut self) -> Result<Option<Message>, &str> {
        
        let frame = self.read_frame();
        if frame.is_err() {
            return Err("Could not read frame");
        }
        let frame = frame.unwrap();
        if frame.is_none() {
            return Ok(None);
        }
        let frame = frame.unwrap();
        if frame.header.fin {
            return Ok(Some(Message::from_frame(frame)));
        }
        if frame.is_control() {
            return  Ok(Some(Message::from_control(frame)));
        }
        let mut frames = vec![frame];
        loop {
            let f = self.read_frame();
            if f.is_err() {
                return Err("Could not read frame");
            }
            let frame = f.unwrap();
            if frame.is_none() {
                continue;
            }
            let frame = frame.unwrap();
            if frame.is_control() {
                continue;
            }
            if frame.header.opcode == CONTINUE {
                if frame.header.fin {
                    break;
                }
                frames.push(frame);
            }
        }
        Ok(Some(Message::from_frames(frames)))
        
    }

    fn send_ping_resp(&mut self, f: &Frame) -> Result<(), &str> {
        let mut fr = Frame::pong(f.contents.clone());
        self.tcp.write(&fr.to_bytes()).ok();
        Ok(())
    }

    fn handle_control(&mut self, frame: &Frame) -> Result<(), &str> {
        println!("Control frame recieved...");
        match frame.header.opcode {
            CLOSE => {
                self.tcp.write(&[0x88, 0]).ok();
                Err("Closed websocket connection")
            }
            PING => {
                self.send_ping_resp(frame)
            }
            _ => {
                Err("Unknown control frame?")
            }
        }
    }

    pub fn read_frame(&mut self) -> Result<Option<Frame>, &str> {
        let mut buffer: [u8; 8192] = [0; 8192];
        
        let res = self.tcp.read(&mut buffer);
        if res.is_err() {            
            return  Ok(None);
        }
        let n = res.ok().unwrap();
        if n == 0 {
            return Err("Connection shut down!");    
        }

        let header = FrameHeader::new(&buffer);

        if header.opcode == 8 {
            self.tcp.write(&[0x88, 0]).ok();
            return Err("Closed websocket connection");
        }


        let frame_len = header.header_len + header.content_len;
        println!("Bytes Recieved: {}, Frame len: {}, Content len: {}, opcode: {}, FIN: {}, masked: {}, mask: {:?}", n, frame_len, header.content_len, header.opcode, header.fin, header.is_masked, header.mask);
        let content = &buffer[(header.header_len as usize)..n];
        let mut frame_data = Vec::from(content);
        let mut recieved = n;
        while (recieved as u64) < frame_len {
            let mut buffer: [u8; 8192] = [0; 8192];
            let n = self.tcp.read(&mut buffer).ok().unwrap();
            recieved += n;
            frame_data.extend(&buffer[0..n]);
        }
        println!("Total recieved bytes: {}, Recieved content: {}", frame_data.len() + header.header_len as usize, frame_data.len());
        
        let frame = Frame::new(header,frame_data);
        if frame.is_control() {
            let r = self.handle_control(&frame);
            if r.is_err() {
                return Err(r.unwrap_err());
            }
        }
        Ok(Some(frame))
    }

    pub fn get_stream(&self) -> &S {
        &self.tcp
    }
}


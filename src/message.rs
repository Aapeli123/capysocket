use crate::frame::{Frame};
use crate::ws_stream::TEXT;
pub struct Message {
    pub control: bool,
    pub msg_type: u8,
    pub contents: Option<Vec<u8>>
}


impl Message {
    pub fn from_frame(f: Frame) -> Message {
        Message{
            control: false,
            contents: Some(f.contents),
            msg_type: f.header.opcode,
        }
    }

    pub fn from_control(f: Frame) -> Message {
        Message{
            control: true,
            msg_type: f.header.opcode,
            contents: None
        }
    }

    pub fn from_frames(frames: Vec<Frame>) -> Message {
        let opcode = frames[0].header.opcode;
        let mut contents = Vec::<u8>::new();
        for mut frame in frames {
            contents.append(&mut frame.contents);
        };
            Message { control: false, contents: Some(contents), msg_type:  opcode}
    }

    pub fn get_text(&self) -> Option<String> {
        if self.contents.is_none() {
            return None
        };
        let content = self.contents.as_ref().unwrap();
        Some(String::from_utf8_lossy(content).to_string())
    }

    pub fn from_text(text: String)  -> Message{
        let bytes = text.as_bytes();
        Message {
            contents: Some(Vec::from(bytes)),
            control: false,
            msg_type: TEXT
        }
    }
    
}
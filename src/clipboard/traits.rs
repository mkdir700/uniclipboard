use anyhow::Result;
use crate::message::Payload;

pub trait ClipboardOperations {
    fn read_text(&mut self) -> Result<String>;
    fn write_text(&mut self, text: &str) -> Result<()>;
    fn read_image(&mut self) -> Result<Payload>;
    fn write_image(&mut self, image: &Payload) -> Result<()>;
}
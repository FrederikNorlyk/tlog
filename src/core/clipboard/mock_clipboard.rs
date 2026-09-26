use crate::core::clipboard::clipboard_backend::ClipboardBackend;

#[derive(Default)]
pub struct MockClipboard {
    last: Option<String>,
}

impl ClipboardBackend for MockClipboard {
    fn set_text(&mut self, text: String) -> Result<(), arboard::Error> {
        self.last = Some(text);
        Ok(())
    }

    fn get_text(&mut self) -> Result<String, arboard::Error> {
        Ok(self.last.clone().unwrap_or_default())
    }
}

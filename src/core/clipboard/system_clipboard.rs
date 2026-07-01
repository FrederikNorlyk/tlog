use crate::core::app_error::AppError;
use crate::core::clipboard::clipboard_backend::ClipboardBackend;
use arboard::Clipboard;

pub struct SystemClipboard {
    inner: Clipboard,
}

impl SystemClipboard {
    /// Creates a new `SystemClipboard`
    ///
    /// # Errors
    /// On some platforms or desktop environments, an error can be returned if clipboards are not supported.
    /// This may be retried.
    pub fn new() -> Result<Self, AppError> {
        Ok(Self {
            inner: Clipboard::new()?,
        })
    }
}

impl ClipboardBackend for SystemClipboard {
    fn set_text(&mut self, text: String) -> Result<(), AppError> {
        self.inner.set_text(text)?;
        Ok(())
    }

    fn get_text(&mut self) -> Result<String, AppError> {
        Ok(self.inner.get_text()?)
    }
}

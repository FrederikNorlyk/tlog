use crate::core::app_error::AppError;

pub trait ClipboardBackend {
    /// Sets the given text
    ///
    /// # Errors
    /// Returns an error if the clipboard backend fails to set the text
    fn set_text(&mut self, text: String) -> Result<(), AppError>;

    /// Sets the text currently on the clipboard
    ///
    /// # Errors
    /// Returns an error if the clipboard backend fails to get the text
    fn get_text(&mut self) -> Result<String, AppError>;
}

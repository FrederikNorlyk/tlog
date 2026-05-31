use ratatui::{DefaultTerminal, Frame};

#[derive(Default)]
pub struct TerminalUserInterface;

impl TerminalUserInterface {
    /// Launches the terminal user interface.
    ///
    /// This method repeatedly renders the UI and waits for keyboard input. It exits
    /// successfully after a key press is received.
    ///
    /// # Errors
    ///
    /// Returns an error if drawing to the terminal fails, or if reading an event
    /// from the terminal input fails.
    pub fn launch(&self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        loop {
            terminal.draw(TerminalUserInterface::render)?;
            if crossterm::event::read()?.is_key_press() {
                break Ok(());
            }
        }
    }

    fn render(frame: &mut Frame) {
        frame.render_widget("Hello world", frame.area());
    }
}

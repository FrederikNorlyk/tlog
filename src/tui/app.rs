use ratatui::{DefaultTerminal, Frame};

pub struct TerminalUserInterface;

impl TerminalUserInterface {
    pub fn default() -> Self {
        Self {}
    }

    pub fn launch(&self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame))?;
            if crossterm::event::read()?.is_key_press() {
                break Ok(());
            }
        }
    }

    fn render(&self, frame: &mut Frame) {
        frame.render_widget("Hello world", frame.area());
    }
}

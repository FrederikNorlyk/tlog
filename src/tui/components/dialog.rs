use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Shadow, Widget};

pub struct Dialog {
    horizontal_constraint: Constraint,
    vertical_constraint: Constraint,
    title_bottom: Option<Line<'static>>,
}

impl Dialog {
    pub fn constrained(horizontal_constraint: Constraint, vertical_constraint: Constraint) -> Self {
        Self {
            horizontal_constraint,
            vertical_constraint,
            title_bottom: None,
        }
    }

    pub fn title_bottom<T: Into<Line<'static>>>(mut self, title: T) -> Self {
        self.title_bottom = Some(title.into());
        self
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) -> Rect {
        let title = " Esc ".blue().bold().into_right_aligned_line();
        let shadow = Shadow::overlay().black().on_yellow();

        let mut block = Block::bordered()
            .title(title)
            .shadow(shadow)
            .bg(Color::LightYellow)
            .fg(Color::DarkGray);

        if let Some(title) = self.title_bottom.clone() {
            block = block.title_bottom(title);
        }

        let centered_area = area.centered(self.horizontal_constraint, self.vertical_constraint);
        let inner = block.inner(centered_area);

        // clear + render block on same area
        Clear.render(centered_area, buf);
        block.render(centered_area, buf);

        inner
    }
}

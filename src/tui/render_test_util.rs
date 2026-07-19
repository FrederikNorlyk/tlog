use ratatui::buffer::{Buffer, Cell};

pub struct RenderTestUtil;

impl RenderTestUtil {
    /// Asserts that the given buffer contains the given expected lines.
    ///
    /// # Panics
    /// If assertion fails
    pub fn assert_eq(expected: Vec<&str>, buffer: &Buffer) {
        let expected_buffer = Buffer::with_lines(expected);
        assert_eq!(Self::flatten(buffer), Self::flatten(&expected_buffer));
    }

    #[must_use]
    fn flatten(buf: &Buffer) -> String {
        let area = buf.area();
        let width = area.width as usize;

        buf.content()
            .chunks(width)
            .map(|row| row.iter().map(Cell::symbol).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

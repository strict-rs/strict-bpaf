//! Screen scraper for the completion tests.
//!
//! A shell draws its completion menu by writing terminal control sequences — cursor motion, scroll
//! regions, line/character edits, full redraws — to the pty rather than the final text. To assert on
//! what the user actually sees, we feed that raw byte stream through a complete VT102 emulator
//! (`vt100`) and read the rendered screen back out.

pub struct Term {
    parser: vt100::Parser,
}

impl Term {
    pub fn new(width: u16, height: u16) -> Self {
        Term {
            parser: vt100::Parser::new(height, width, 0),
        }
    }

    pub fn process(&mut self, buf: &[u8]) {
        self.parser.process(buf);
    }

    /// The visible screen as plain text, with trailing whitespace trimmed from each line and
    /// trailing blank lines removed — matching how the completion snapshots are written.
    pub fn render(&mut self) -> String {
        let screen = self.parser.screen();
        let (rows, cols) = screen.size();
        let mut res = String::new();
        for row in 0..rows {
            for col in 0..cols {
                match screen.cell(row, col) {
                    Some(cell) if cell.has_contents() => res.push_str(cell.contents()),
                    _ => res.push(' '),
                }
            }
            res.truncate(res.trim_end().len());
            res.push('\n');
        }
        res.truncate(res.trim_end().len());
        res
    }
}

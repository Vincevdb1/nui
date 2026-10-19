use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{prelude::*, widgets::*};

/// A single-line text field with a cursor, measured in chars.
#[derive(Default, Clone)]
pub struct TextInput {
    value: String,
    cursor: usize,
}

impl From<String> for TextInput {
    fn from(value: String) -> Self {
        let cursor = value.chars().count();
        Self { value, cursor }
    }
}

impl TextInput {
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Applies an editing or cursor key; returns false for keys it ignores.
    pub fn handle(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let len = self.value.chars().count();
        match key.code {
            KeyCode::Left if ctrl || alt => self.cursor = self.word_start(),
            KeyCode::Right if ctrl || alt => self.cursor = self.word_end(),
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(len),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = len,
            KeyCode::Char('a') if ctrl => self.cursor = 0,
            KeyCode::Char('e') if ctrl => self.cursor = len,
            // Terminals send Ctrl+Backspace as ^H, ^W or Backspace with a modifier.
            KeyCode::Backspace if ctrl || alt => self.delete_to(self.word_start()),
            KeyCode::Char('h' | 'w') if ctrl => self.delete_to(self.word_start()),
            KeyCode::Backspace => self.delete_to(self.cursor.saturating_sub(1)),
            KeyCode::Delete if ctrl => self.delete_to(self.word_end()),
            KeyCode::Char('d') if alt => self.delete_to(self.word_end()),
            KeyCode::Delete => self.delete_to((self.cursor + 1).min(len)),
            KeyCode::Char('u') if ctrl => self.delete_to(0),
            KeyCode::Char('k') if ctrl => self.delete_to(len),
            KeyCode::Char(c) if !ctrl && !alt => {
                let at = self.byte(self.cursor);
                self.value.insert(at, c);
                self.cursor += 1;
            }
            _ => return false,
        }
        true
    }

    /// Draws the field, scrolled so the cursor stays visible, and places the
    /// terminal cursor in it when focused.
    pub fn render(&self, frame: &mut Frame, area: Rect, block: Block, focused: bool) {
        let inner = block.inner(area);
        let width = inner.width.saturating_sub(1) as usize;
        let scroll = self.cursor.saturating_sub(width);
        frame.render_widget(
            Paragraph::new(self.value.as_str())
                .block(block)
                .scroll((0, scroll as u16)),
            area,
        );
        if focused && inner.width > 0 {
            let x = inner.x + (self.cursor - scroll) as u16;
            frame.set_cursor_position((x, inner.y));
        }
    }

    fn byte(&self, index: usize) -> usize {
        self.value
            .char_indices()
            .nth(index)
            .map_or(self.value.len(), |(i, _)| i)
    }

    /// Removes the chars between the cursor and `to`, leaving the cursor at the start.
    fn delete_to(&mut self, to: usize) {
        let (start, end) = (self.cursor.min(to), self.cursor.max(to));
        let range = self.byte(start)..self.byte(end);
        self.value.replace_range(range, "");
        self.cursor = start;
    }

    fn word_start(&self) -> usize {
        let chars: Vec<char> = self.value.chars().collect();
        let mut i = self.cursor;
        while i > 0 && is_separator(chars[i - 1]) {
            i -= 1;
        }
        while i > 0 && !is_separator(chars[i - 1]) {
            i -= 1;
        }
        i
    }

    fn word_end(&self) -> usize {
        let chars: Vec<char> = self.value.chars().collect();
        let mut i = self.cursor;
        while i < chars.len() && is_separator(chars[i]) {
            i += 1;
        }
        while i < chars.len() && !is_separator(chars[i]) {
            i += 1;
        }
        i
    }
}

/// Word boundaries also split flake references like `github:owner/repo`.
fn is_separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, '/' | '.' | ':' | '-')
}

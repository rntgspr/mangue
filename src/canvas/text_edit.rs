//! `TextEdit`: GPUI-independent text, selection and IME composition state of
//! a text input (byte offsets into `text`; UTF-16 conversions for the
//! platform input handler).

use std::ops::Range;

/// Text with a selection (`selected`, empty = caret) whose moving end is
/// `start` when `reversed`, and an optional IME composition range.
#[derive(Debug, Clone, PartialEq)]
pub struct TextEdit {
    pub text: String,
    pub selected: Range<usize>,
    pub reversed: bool,
    pub marked: Option<Range<usize>>,
    /// Whether newlines are kept (multi-line) or dropped.
    pub multiline: bool,
}

impl TextEdit {
    /// Starts editing `text` with the caret at its end.
    pub fn new(text: &str, multiline: bool) -> Self {
        let text = if multiline {
            text.to_string()
        } else {
            text.replace('\n', " ")
        };
        let end = text.len();

        TextEdit {
            text,
            selected: end..end,
            reversed: false,
            marked: None,
            multiline,
        }
    }

    /// Offset of the caret (the moving end of the selection).
    pub fn cursor(&self) -> usize {
        if self.reversed {
            self.selected.start
        } else {
            self.selected.end
        }
    }

    /// Collapses the selection to a caret at `offset`.
    pub fn move_to(&mut self, offset: usize) {
        self.selected = offset..offset;
        self.reversed = false;
    }

    /// Extends the selection's moving end to `offset`.
    pub fn select_to(&mut self, offset: usize) {
        if self.reversed {
            self.selected.start = offset;
        } else {
            self.selected.end = offset;
        }

        if self.selected.end < self.selected.start {
            self.reversed = !self.reversed;
            self.selected = self.selected.end..self.selected.start;
        }
    }

    /// Selects the whole text.
    pub fn select_all(&mut self) {
        self.selected = 0..self.text.len();
        self.reversed = false;
    }

    /// The selected text.
    pub fn selected_text(&self) -> &str {
        &self.text[self.selected.clone()]
    }

    /// Replaces `range` (else the composition, else the selection) with
    /// `new_text` and puts the caret after it; ends any composition.
    pub fn replace(&mut self, range: Option<Range<usize>>, new_text: &str) {
        let range = self.target(range);
        let new_text = self.accepted(new_text);

        self.text.replace_range(range.clone(), &new_text);
        self.move_to(range.start + new_text.len());
        self.marked = None;
    }

    /// Replaces like `replace` but marks the inserted text as an IME
    /// composition; `selected` is relative to the inserted text.
    pub fn replace_and_mark(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        selected: Option<Range<usize>>,
    ) {
        let range = self.target(range);
        let new_text = self.accepted(new_text);

        self.text.replace_range(range.clone(), &new_text);
        self.marked = (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());

        let end = range.start + new_text.len();
        self.selected = selected
            .map(|s| (range.start + s.start).min(end)..(range.start + s.end).min(end))
            .unwrap_or(end..end);
        self.reversed = false;
    }

    /// Range a replacement applies to: explicit, else composition, else selection.
    fn target(&self, range: Option<Range<usize>>) -> Range<usize> {
        let len = self.text.len();
        let range = range
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selected.clone());

        range.start.min(len)..range.end.min(len)
    }

    /// `text` as inserted: newlines become spaces in single-line mode.
    fn accepted(&self, text: &str) -> String {
        if self.multiline {
            text.replace("\r\n", "\n")
        } else {
            text.replace(['\r', '\n'], " ")
        }
    }

    /// Start of the character before `offset` (0 at the start).
    pub fn prev_char(&self, offset: usize) -> usize {
        self.text[..offset]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    /// End of the character after `offset` (the length at the end).
    pub fn next_char(&self, offset: usize) -> usize {
        self.text[offset..]
            .chars()
            .next()
            .map_or(offset, |c| offset + c.len_utf8())
    }

    /// Start of the word before `offset` (skipping separators first).
    pub fn prev_word(&self, offset: usize) -> usize {
        let mut chars = self.text[..offset].char_indices().rev().peekable();

        while chars.next_if(|(_, c)| !c.is_alphanumeric()).is_some() {}

        let mut start = chars.peek().map_or(0, |(i, _)| *i);
        while let Some((i, _)) = chars.next_if(|(_, c)| c.is_alphanumeric()) {
            start = i;
        }

        start
    }

    /// End of the word after `offset` (skipping separators first).
    pub fn next_word(&self, offset: usize) -> usize {
        let rest = &self.text[offset..];
        let mut chars = rest.char_indices().peekable();

        while chars.next_if(|(_, c)| !c.is_alphanumeric()).is_some() {}
        while chars.next_if(|(_, c)| c.is_alphanumeric()).is_some() {}

        offset + chars.peek().map_or(rest.len(), |(i, _)| *i)
    }

    /// Start of the line containing `offset`.
    pub fn line_start(&self, offset: usize) -> usize {
        self.text[..offset].rfind('\n').map_or(0, |i| i + 1)
    }

    /// End (before the newline) of the line containing `offset`.
    pub fn line_end(&self, offset: usize) -> usize {
        self.text[offset..]
            .find('\n')
            .map_or(self.text.len(), |i| offset + i)
    }

    /// Same character column on the previous line (the text start on the first).
    pub fn line_up(&self, offset: usize) -> usize {
        let start = self.line_start(offset);
        if start == 0 {
            return 0;
        }

        let column = self.text[start..offset].chars().count();
        let above = self.line_start(start - 1);
        self.at_column(above, column)
    }

    /// Same character column on the next line (the text end on the last).
    pub fn line_down(&self, offset: usize) -> usize {
        let end = self.line_end(offset);
        if end == self.text.len() {
            return end;
        }

        let column = self.text[self.line_start(offset)..offset].chars().count();
        self.at_column(end + 1, column)
    }

    /// Offset `column` characters into the line starting at `start`,
    /// clamped to that line's end.
    fn at_column(&self, start: usize, column: usize) -> usize {
        let end = self.line_end(start);

        self.text[start..end]
            .char_indices()
            .nth(column)
            .map_or(end, |(i, _)| start + i)
    }

    /// Byte offsets of each line's start and end (newlines excluded).
    pub fn lines(&self) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        let mut start = 0;

        for (i, c) in self.text.char_indices() {
            if c == '\n' {
                out.push(start..i);
                start = i + 1;
            }
        }
        out.push(start..self.text.len());

        out
    }

    /// UTF-16 offset of byte offset `offset`.
    pub fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .chars()
            .map(char::len_utf16)
            .sum()
    }

    /// Byte offset of UTF-16 offset `offset` (clamped to the text).
    pub fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;

        for (i, c) in self.text.char_indices() {
            if utf16 >= offset {
                return i;
            }
            utf16 += c.len_utf16();
        }

        self.text.len()
    }

    /// `range` converted to UTF-16 offsets.
    pub fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    /// `range` converted from UTF-16 offsets.
    pub fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replacing the selection inserts text and leaves the caret after it.
    #[test]
    fn replace_selection() {
        let mut edit = TextEdit::new("Hello world", false);

        edit.move_to(6);
        edit.select_to(11);
        assert_eq!(edit.selected_text(), "world");

        edit.replace(None, "there");
        assert_eq!(edit.text, "Hello there");
        assert_eq!(edit.selected, 11..11);
    }

    /// Selecting past the anchor flips the selection direction.
    #[test]
    fn select_backwards() {
        let mut edit = TextEdit::new("abcd", false);

        edit.move_to(2);
        edit.select_to(0);
        assert_eq!(edit.selected, 0..2);
        assert!(edit.reversed);
        assert_eq!(edit.cursor(), 0);
    }

    /// Single-line inputs turn newlines into spaces; multi-line keeps them.
    #[test]
    fn newline_handling() {
        let mut single = TextEdit::new("a", false);
        single.replace(None, "\nb");
        assert_eq!(single.text, "a b");

        let mut multi = TextEdit::new("a", true);
        multi.replace(None, "\nb");
        assert_eq!(multi.text, "a\nb");
        assert_eq!(multi.lines(), vec![0..1, 2..3]);
    }

    /// Word motions skip separators, then a run of word characters.
    #[test]
    fn word_motion() {
        let edit = TextEdit::new("foo  bar-baz", false);

        assert_eq!(edit.next_word(0), 3);
        assert_eq!(edit.next_word(3), 8);
        assert_eq!(edit.prev_word(12), 9);
        assert_eq!(edit.prev_word(9), 5);
        assert_eq!(edit.prev_word(5), 0);
    }

    /// Vertical motion keeps the character column, clamped to the line.
    #[test]
    fn line_motion() {
        let edit = TextEdit::new("abcd\nx\nwxyz", true);

        assert_eq!(edit.line_down(3), 6);
        assert_eq!(edit.line_down(6), 8);
        assert_eq!(edit.line_up(9), 6);
        assert_eq!(edit.line_up(2), 0);
        assert_eq!(edit.line_down(9), 11);
        assert_eq!(edit.line_start(9), 7);
        assert_eq!(edit.line_end(0), 4);
    }

    /// IME composition marks the inserted text; a final replace commits it.
    #[test]
    fn ime_composition() {
        let mut edit = TextEdit::new("a", false);

        edit.replace_and_mark(None, "e", Some(1..1));
        assert_eq!(edit.marked, Some(1..2));

        edit.replace(None, "\u{e9}");
        assert_eq!(edit.text, "a\u{e9}");
        assert_eq!(edit.marked, None);
        assert_eq!(edit.offset_to_utf16(edit.text.len()), 2);
        assert_eq!(edit.offset_from_utf16(2), 3);
    }
}

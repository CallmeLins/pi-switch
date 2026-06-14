use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputPolicy {
    Any,
    Digits,
    Url,
}

impl TextInputPolicy {
    pub fn allows(&self, c: char) -> bool {
        match self {
            TextInputPolicy::Any => true,
            TextInputPolicy::Digits => c.is_ascii_digit(),
            TextInputPolicy::Url => !c.is_whitespace() && !c.is_control(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEditCommand {
    MoveLeft,
    MoveRight,
    MoveLineStart,
    MoveLineEnd,
    MoveWordLeft,
    MoveWordRight,
    DeleteBackward,
    DeleteForward,
    DeleteToLineStart,
    DeleteToLineEnd,
    DeleteWordBackward,
    Insert(char),
}

impl TextEditCommand {
    pub fn from_key(key: KeyEvent) -> Option<Self> {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        if control {
            return match key.code {
                KeyCode::Char('a' | 'A') => Some(Self::MoveLineStart),
                KeyCode::Char('b' | 'B') => Some(Self::MoveLeft),
                KeyCode::Char('d' | 'D') => Some(Self::DeleteForward),
                KeyCode::Char('e' | 'E') => Some(Self::MoveLineEnd),
                KeyCode::Char('f' | 'F') => Some(Self::MoveRight),
                KeyCode::Char('k' | 'K') => Some(Self::DeleteToLineEnd),
                KeyCode::Char('u' | 'U') => Some(Self::DeleteToLineStart),
                KeyCode::Char('w' | 'W') => Some(Self::DeleteWordBackward),
                _ => None,
            };
        }

        if alt {
            return match key.code {
                KeyCode::Backspace => Some(Self::DeleteWordBackward),
                KeyCode::Char('b' | 'B') => Some(Self::MoveWordLeft),
                KeyCode::Char('f' | 'F') => Some(Self::MoveWordRight),
                _ => None,
            };
        }

        match key.code {
            KeyCode::Left => Some(Self::MoveLeft),
            KeyCode::Right => Some(Self::MoveRight),
            KeyCode::Home => Some(Self::MoveLineStart),
            KeyCode::End => Some(Self::MoveLineEnd),
            KeyCode::Backspace => Some(Self::DeleteBackward),
            KeyCode::Delete => Some(Self::DeleteForward),
            KeyCode::Char(c) if !c.is_control() => Some(Self::Insert(c)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextInput {
    pub value: String,
    pub cursor: usize,
    pub policy: TextInputPolicy,
}

impl Default for TextInput {
    fn default() -> Self {
        Self { value: String::new(), cursor: 0, policy: TextInputPolicy::Any }
    }
}

impl TextInput {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self { value, cursor, policy: TextInputPolicy::Any }
    }

    pub fn with_policy(mut self, policy: TextInputPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn set(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor = self.value.chars().count();
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    pub fn len_chars(&self) -> usize {
        self.value.chars().count()
    }

    pub fn byte_index(line: &str, col: usize) -> usize {
        line.char_indices()
            .nth(col)
            .map(|(i, _)| i)
            .unwrap_or(line.len())
    }

    /// Returns Some(changed) when the key was consumed as an edit command.
    pub fn apply_key(&mut self, key: KeyEvent) -> Option<bool> {
        let command = TextEditCommand::from_key(key)?;
        Some(self.apply_command(command))
    }

    fn apply_command(&mut self, command: TextEditCommand) -> bool {
        self.clamp_cursor();
        match command {
            TextEditCommand::MoveLeft => self.move_left(),
            TextEditCommand::MoveRight => self.move_right(),
            TextEditCommand::MoveLineStart => self.move_home(),
            TextEditCommand::MoveLineEnd => self.move_end(),
            TextEditCommand::MoveWordLeft => self.move_word_left(),
            TextEditCommand::MoveWordRight => self.move_word_right(),
            TextEditCommand::DeleteBackward => self.backspace(),
            TextEditCommand::DeleteForward => self.delete(),
            TextEditCommand::DeleteToLineStart => self.delete_to_line_start(),
            TextEditCommand::DeleteToLineEnd => self.delete_to_line_end(),
            TextEditCommand::DeleteWordBackward => self.delete_word_backward(),
            TextEditCommand::Insert(c) => self.insert_char(c),
        }
    }

    fn clamp_cursor(&mut self) {
        self.cursor = self.cursor.min(self.len_chars());
    }

    fn move_left(&mut self) -> bool {
        let before = self.cursor;
        self.cursor = self.cursor.saturating_sub(1);
        self.cursor != before
    }

    fn move_right(&mut self) -> bool {
        let before = self.cursor;
        self.cursor = (self.cursor + 1).min(self.len_chars());
        self.cursor != before
    }

    fn move_home(&mut self) -> bool {
        let before = self.cursor;
        self.cursor = 0;
        self.cursor != before
    }

    fn move_end(&mut self) -> bool {
        let before = self.cursor;
        self.cursor = self.len_chars();
        self.cursor != before
    }

    fn move_word_left(&mut self) -> bool {
        let before = self.cursor;
        self.cursor = previous_word_boundary(&self.value, self.cursor);
        self.cursor != before
    }

    fn move_word_right(&mut self) -> bool {
        let before = self.cursor;
        self.cursor = next_word_boundary(&self.value, self.cursor);
        self.cursor != before
    }

    fn insert_char(&mut self, c: char) -> bool {
        if !self.policy.allows(c) {
            return false;
        }
        let idx = Self::byte_index(&self.value, self.cursor);
        self.value.insert(idx, c);
        self.cursor += 1;
        true
    }

    fn backspace(&mut self) -> bool {
        if self.cursor == 0 || self.value.is_empty() {
            return false;
        }
        let start = Self::byte_index(&self.value, self.cursor.saturating_sub(1));
        let end = Self::byte_index(&self.value, self.cursor);
        self.value.replace_range(start..end, "");
        self.cursor = self.cursor.saturating_sub(1);
        true
    }

    fn delete(&mut self) -> bool {
        let len = self.len_chars();
        if self.value.is_empty() || self.cursor >= len {
            return false;
        }
        let start = Self::byte_index(&self.value, self.cursor);
        let end = Self::byte_index(&self.value, self.cursor + 1);
        self.value.replace_range(start..end, "");
        true
    }

    fn delete_to_line_start(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let end = Self::byte_index(&self.value, self.cursor);
        self.value.replace_range(0..end, "");
        self.cursor = 0;
        true
    }

    fn delete_to_line_end(&mut self) -> bool {
        if self.cursor >= self.len_chars() {
            return false;
        }
        let start = Self::byte_index(&self.value, self.cursor);
        self.value.replace_range(start.., "");
        true
    }

    fn delete_word_backward(&mut self) -> bool {
        let start_cursor = previous_word_boundary(&self.value, self.cursor);
        if start_cursor == self.cursor {
            return false;
        }
        let start = Self::byte_index(&self.value, start_cursor);
        let end = Self::byte_index(&self.value, self.cursor);
        self.value.replace_range(start..end, "");
        self.cursor = start_cursor;
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharKind {
    Whitespace,
    Word,
    Other,
}

fn char_kind(c: char) -> CharKind {
    if c.is_whitespace() {
        CharKind::Whitespace
    } else if c.is_alphanumeric() || c == '_' {
        CharKind::Word
    } else {
        CharKind::Other
    }
}

fn previous_word_boundary(text: &str, cursor: usize) -> usize {
    let chars = text.chars().collect::<Vec<_>>();
    let mut idx = cursor.min(chars.len());

    while idx > 0 && char_kind(chars[idx - 1]) == CharKind::Whitespace {
        idx -= 1;
    }
    if idx == 0 {
        return 0;
    }
    let target = char_kind(chars[idx - 1]);
    while idx > 0 && char_kind(chars[idx - 1]) == target {
        idx -= 1;
    }
    idx
}

fn next_word_boundary(text: &str, cursor: usize) -> usize {
    let chars = text.chars().collect::<Vec<_>>();
    let mut idx = cursor.min(chars.len());

    while idx < chars.len() && char_kind(chars[idx]) == CharKind::Whitespace {
        idx += 1;
    }
    if idx >= chars.len() {
        return chars.len();
    }
    let target = char_kind(chars[idx]);
    while idx < chars.len() && char_kind(chars[idx]) == target {
        idx += 1;
    }
    idx
}

/// Visible window of `value` for a single-line input of `width` cells,
/// returning (visible_text, cursor_x_offset).
pub fn visible_text_window(value: &str, cursor: usize, width: u16) -> (String, u16) {
    let width = width as usize;
    if width == 0 {
        return (String::new(), 0);
    }
    let chars: Vec<char> = value.chars().collect();
    let cursor = cursor.min(chars.len());

    let start = if cursor >= width {
        cursor + 1 - width
    } else {
        0
    };
    let end = (start + width).min(chars.len());
    let visible: String = chars[start..end].iter().collect();
    (visible, (cursor - start) as u16)
}

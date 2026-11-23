//! The physical keys behind the text just typed, so the last word can be retyped in another
//! layout. Keys are stored, not characters: the same key is `g` in English and `п` in
//! Ukrainian or Russian, which is the whole point.

/// One recorded key press with the modifier state that shaped its character.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub vk: u16,
    pub scan: u16,
    pub shift: bool,
    pub caps: bool,
}

/// What a key press does to the buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Produces a character that differs between layouts: letters, digits, punctuation keys.
    Word,
    /// Ends a word but stays part of what a fix retypes (space).
    Separator,
    /// Deletes the previous key's character.
    Backspace,
    /// The caret may have moved or the text changed in ways we cannot follow.
    Reset,
    /// Modifiers and other keys that do not change the text.
    Ignore,
}

/// The last word and the separators typed after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastWord {
    pub word: Vec<Key>,
    pub trailing: Vec<Key>,
}

/// Bounded history of keys since the last reset.
pub struct Buffer {
    keys: Vec<(Key, bool)>,
    limit: usize,
}

impl Buffer {
    pub fn new(limit: usize) -> Self {
        Self {
            keys: Vec::new(),
            limit,
        }
    }

    pub fn push(&mut self, kind: Kind, key: Key) {
        match kind {
            Kind::Word | Kind::Separator => {
                if self.keys.len() == self.limit {
                    self.keys.remove(0);
                }
                self.keys.push((key, kind == Kind::Separator));
            }
            Kind::Backspace => {
                self.keys.pop();
            }
            Kind::Reset => self.keys.clear(),
            Kind::Ignore => {}
        }
    }

    pub fn clear(&mut self) {
        self.keys.clear();
    }

    /// The trailing run of separators and the word before it, if there is a word.
    pub fn last_word(&self) -> Option<LastWord> {
        let trailing_start = self
            .keys
            .iter()
            .rposition(|(_, separator)| !separator)
            .map_or(0, |index| index + 1);
        let word_start = self.keys[..trailing_start]
            .iter()
            .rposition(|(_, separator)| *separator)
            .map_or(0, |index| index + 1);
        if word_start == trailing_start {
            return None;
        }
        let keys =
            |range: std::ops::Range<usize>| self.keys[range].iter().map(|(key, _)| *key).collect();
        Some(LastWord {
            word: keys(word_start..trailing_start),
            trailing: keys(trailing_start..self.keys.len()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(vk: u8) -> Key {
        Key {
            vk: u16::from(vk),
            scan: 0,
            shift: false,
            caps: false,
        }
    }

    fn typed(buffer: &mut Buffer, text: &str) {
        for byte in text.bytes() {
            match byte {
                b' ' => buffer.push(Kind::Separator, key(b' ')),
                b'<' => buffer.push(Kind::Backspace, key(8)),
                b'|' => buffer.push(Kind::Reset, key(0)),
                _ => buffer.push(Kind::Word, key(byte)),
            }
        }
    }

    fn word(text: &str) -> Vec<Key> {
        text.bytes().map(key).collect()
    }

    #[test]
    fn last_word_keeps_trailing_spaces_and_follows_backspace_and_reset() {
        let mut buffer = Buffer::new(64);
        typed(&mut buffer, "ghbdtn vbh  ");
        assert_eq!(
            buffer.last_word(),
            Some(LastWord {
                word: word("vbh"),
                trailing: word("  "),
            })
        );
        // Backspacing into the word shortens it; backspacing past it reaches the previous word.
        typed(&mut buffer, "<<<");
        assert_eq!(buffer.last_word().unwrap().word, word("vb"));
        typed(&mut buffer, "<<");
        assert_eq!(buffer.last_word().unwrap().word, word("ghbdtn"));
        // A caret move forgets everything typed before it.
        typed(&mut buffer, "|  ");
        assert_eq!(buffer.last_word(), None);
    }

    #[test]
    fn history_is_bounded_by_dropping_the_oldest_keys() {
        let mut buffer = Buffer::new(4);
        typed(&mut buffer, "abcdef");
        assert_eq!(buffer.last_word().unwrap().word, word("cdef"));
    }
}

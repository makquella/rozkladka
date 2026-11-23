//! Automatic mode: after a word ends, decide whether it was typed in the wrong layout.
//!
//! Windows' own spell checkers are the dictionaries: they know every installed language's
//! word forms and the person's added words, and cost nothing to ship. A word is fixed only
//! when it is *not* a word in the layout it was typed in *and* the same keys spell a real word
//! in the other layout.

use crate::layouts::Layout;
use std::collections::HashMap;
use windows::Win32::{
    Foundation::S_OK,
    Globalization::{ISpellChecker, ISpellCheckerFactory, LCIDToLocaleName, SpellCheckerFactory},
    System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
};
use windows::core::HSTRING;

/// Which side of the decision a word is checked against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The layout the word was typed in.
    Typed,
    /// The layout the fix would switch to.
    Other,
}

/// Single letters are left alone: too many are words by accident ("f" is "а", "d" is "в").
const MIN_LETTERS: usize = 2;

/// Whether `shown` (what the keys typed) should become `alt` (what they type in the other
/// layout). `is_word` answers from a dictionary, `None` when that language has none.
///
/// Punctuation keys are letters in Cyrillic layouts (`;` is `ж`, `,` is `б`), so the word is
/// trimmed by what is a letter in `alt`, and `shown` is checked over the same keys.
pub fn should_fix(
    shown: &str,
    alt: &str,
    mut is_word: impl FnMut(Side, &str) -> Option<bool>,
) -> bool {
    let shown: Vec<char> = shown.chars().collect();
    let alt: Vec<char> = alt.chars().collect();
    if shown.len() != alt.len() {
        return false;
    }
    let Some(start) = alt.iter().position(|c| c.is_alphabetic()) else {
        return false;
    };
    let end = alt.iter().rposition(|c| c.is_alphabetic()).unwrap_or(start) + 1;
    let inside_word = |c: &char| c.is_alphabetic() || matches!(c, '-' | '\'' | 'ʼ' | '’');
    if !alt[start..end].iter().all(inside_word) || end - start < MIN_LETTERS {
        return false;
    }
    let alt_word: String = alt[start..end].iter().collect();
    let shown_word: String = shown[start..end].iter().collect();
    is_word(Side::Other, &alt_word) == Some(true)
        && is_word(Side::Typed, &shown_word) == Some(false)
}

/// The BCP 47 tag of a layout's language, e.g. `ru-RU`, as the spell checkers name them.
pub fn language_tag(layout: Layout) -> Option<String> {
    let mut buffer = [0_u16; 85];
    // SAFETY: The buffer is LOCALE_NAME_MAX_LENGTH wide characters.
    let written = unsafe { LCIDToLocaleName(layout.language_id(), Some(&mut buffer), 0) };
    if written <= 1 {
        return None;
    }
    String::from_utf16(&buffer[..written as usize - 1]).ok()
}

/// Spell checkers per language, created on first use. Lives on the action thread, which
/// initializes COM.
pub struct Speller {
    factory: ISpellCheckerFactory,
    checkers: HashMap<String, Option<ISpellChecker>>,
}

impl Speller {
    pub fn new() -> Option<Self> {
        // SAFETY: COM is initialized on the calling thread before this is called.
        let factory =
            unsafe { CoCreateInstance(&SpellCheckerFactory, None, CLSCTX_INPROC_SERVER) }.ok()?;
        Some(Self {
            factory,
            checkers: HashMap::new(),
        })
    }

    /// Whether `word` is spelled correctly in `tag`; `None` without a dictionary for it.
    pub fn is_word(&mut self, tag: &str, word: &str) -> Option<bool> {
        let factory = &self.factory;
        let checker = self
            .checkers
            .entry(tag.to_owned())
            .or_insert_with(|| {
                let tag = HSTRING::from(tag);
                // SAFETY: Plain COM calls on the owning thread.
                unsafe {
                    if !factory.IsSupported(&tag).ok()?.as_bool() {
                        return None;
                    }
                    factory.CreateSpellChecker(&tag).ok()
                }
            })
            .as_ref()?;
        // SAFETY: Plain COM calls; the enumerator owns its errors.
        unsafe {
            let errors = checker.Check(&HSTRING::from(word)).ok()?;
            let mut error = None;
            // S_OK with an error means a misspelling; S_FALSE means the text is clean.
            Some(!(errors.Next(&mut error) == S_OK && error.is_some()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny stand-in dictionary: Russian words on the other side, English on the typed side.
    fn dictionary(side: Side, word: &str) -> Option<bool> {
        let words: &[&str] = match side {
            Side::Other => &["привет", "жук", "кто-то", "ты", "щл"],
            Side::Typed => &["hello", "er", "ok"],
        };
        Some(words.contains(&word.to_lowercase().as_str()))
    }

    #[test]
    fn fixes_only_nonwords_whose_other_layout_spelling_is_a_word() {
        assert!(should_fix("ghbdtn", "привет", dictionary));
        assert!(should_fix("Ghbdtn", "Привет", dictionary));
        // Trailing punctuation is outside the word: `?` is the comma key in Russian.
        assert!(should_fix("ghbdtn?", "привет,", dictionary));
        // `;` is `ж`: the word is cut by the Cyrillic letters, so `;er` is checked, not `er`.
        assert!(should_fix(";er", "жук", dictionary));
        assert!(should_fix("rnj-nj", "кто-то", dictionary));
        // A real English word stays even if the other layout also spells a word.
        assert!(!should_fix("ok", "щл", dictionary));
        // Not a word on either side: leave it (code, names, typos).
        assert!(!should_fix("println", "зкштедт", dictionary));
        // Too short to judge.
        assert!(!should_fix("f", "а", dictionary));
        // No dictionary for a language: never guess.
        assert!(!should_fix("ghbdtn", "привет", |_, _| None));
    }

    #[test]
    fn windows_spell_checkers_tell_words_from_wrong_layout_text() {
        // SAFETY: Initializes COM for this test thread.
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_MULTITHREADED,
            );
        }
        let Some(mut speller) = Speller::new() else {
            eprintln!("skipped: no spell checker factory");
            return;
        };
        for (tag, word, typo) in [
            ("en-US", "hello", "ghbdtn"),
            ("ru-RU", "привет", "руддщ"),
            ("uk-UA", "привіт", "руддщ"),
        ] {
            match speller.is_word(tag, word) {
                None => eprintln!("skipped: no {tag} dictionary"),
                Some(known) => {
                    assert!(known, "{tag}: {word}");
                    assert_eq!(speller.is_word(tag, typo), Some(false), "{tag}: {typo}");
                }
            }
        }
    }
}

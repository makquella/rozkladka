//! Installed keyboard layouts and "the same keys in the other layout" conversion.
//!
//! Characters come from Windows' own layout tables (`ToUnicodeEx`, `VkKeyScanExW`), so any
//! pair the person has installed works: English with Ukrainian, Russian, Belarusian and so on.

use crate::buffer::Key;
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::{
        Input::KeyboardAndMouse::{
            GetKeyboardLayout, GetKeyboardLayoutList, HKL, ToUnicodeEx, VK_CAPITAL, VK_SHIFT,
            VkKeyScanExW,
        },
        WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId, PostMessageW, WM_INPUTLANGCHANGEREQUEST,
        },
    },
};

const ENGLISH: u16 = 0x09;
/// Preferred partner for English when several non-English layouts are installed and the
/// current one is English: Ukrainian, then Russian, then Belarusian.
const PREFERRED: [u16; 3] = [0x22, 0x19, 0x23];
/// `ToUnicodeEx` flag: do not change the keyboard state (Windows 10 1607 and later).
const NO_STATE_CHANGE: u32 = 0x4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout(pub HKL);

// SAFETY: an HKL is a process-independent layout identifier, not a pointer we dereference.
unsafe impl Send for Layout {}

impl Layout {
    /// The layout's language identifier (LANGID), e.g. 0x0419 for Russian.
    pub fn language_id(self) -> u32 {
        (self.0.0 as usize & 0xFFFF) as u32
    }

    fn language(self) -> u16 {
        (self.0.0 as usize & 0x3FF) as u16
    }
}

/// The two layouts a fix switches between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pair {
    pub english: Layout,
    pub other: Layout,
}

impl Pair {
    pub fn opposite(self, source: Layout) -> Layout {
        if source == self.english {
            self.other
        } else {
            self.english
        }
    }
}

pub fn installed() -> Vec<Layout> {
    // SAFETY: The first call sizes the list; the second fills a buffer of that size.
    unsafe {
        let count = GetKeyboardLayoutList(None);
        let mut list = vec![HKL::default(); count.max(0) as usize];
        let filled = GetKeyboardLayoutList(Some(&mut list));
        list.truncate(filled.max(0) as usize);
        list.into_iter().map(Layout).collect()
    }
}

/// English and the layout to pair it with: the current one when it is not English,
/// otherwise the preferred installed partner.
pub fn pair(installed: &[Layout], current: Layout) -> Option<Pair> {
    let english = *installed
        .iter()
        .find(|layout| layout.language() == ENGLISH)?;
    let other = if current != english && installed.contains(&current) {
        current
    } else {
        PREFERRED
            .iter()
            .find_map(|language| {
                installed
                    .iter()
                    .find(|layout| layout.language() == *language)
            })
            .or_else(|| installed.iter().find(|layout| **layout != english))
            .copied()?
    };
    Some(Pair { english, other })
}

/// The focused window and the layout its thread is typing with.
pub fn foreground() -> (HWND, Layout) {
    // SAFETY: Plain queries; a missing window yields thread 0, whose layout is the default.
    unsafe {
        let window = GetForegroundWindow();
        let thread = GetWindowThreadProcessId(window, None);
        (window, Layout(GetKeyboardLayout(thread)))
    }
}

/// Asks the focused window to switch its input layout.
pub fn switch(window: HWND, layout: Layout) {
    // SAFETY: A posted request; the target window applies it on its own thread.
    unsafe {
        let _ = PostMessageW(
            Some(window),
            WM_INPUTLANGCHANGEREQUEST,
            WPARAM(0),
            LPARAM(layout.0.0 as isize),
        );
    }
}

/// The character(s) a recorded key produces in `layout`, or `None` for dead and unmapped keys.
pub fn key_text(key: Key, layout: Layout) -> Option<String> {
    let mut state = [0_u8; 256];
    if key.shift {
        state[usize::from(VK_SHIFT.0)] = 0x80;
    }
    if key.caps {
        state[usize::from(VK_CAPITAL.0)] = 0x01;
    }
    let mut buffer = [0_u16; 8];
    // SAFETY: Both buffers are valid for the call; the flag leaves dead-key state untouched.
    let written = unsafe {
        ToUnicodeEx(
            u32::from(key.vk),
            u32::from(key.scan),
            &state,
            &mut buffer,
            NO_STATE_CHANGE,
            Some(layout.0),
        )
    };
    if written <= 0 {
        return None;
    }
    String::from_utf16(&buffer[..written as usize]).ok()
}

/// The text these keys produce in `layout`; `None` if any key produces nothing.
pub fn keys_text(keys: &[Key], layout: Layout) -> Option<String> {
    keys.iter().map(|key| key_text(*key, layout)).collect()
}

/// The key (with Shift, never Ctrl or Alt) that types `character` in `layout`.
fn key_for(character: char, layout: Layout) -> Option<Key> {
    let unit = u16::try_from(u32::from(character)).ok()?;
    // SAFETY: Pure table lookup.
    let found = unsafe { VkKeyScanExW(unit, layout.0) };
    if found == -1 {
        return None;
    }
    let modifiers = (found as u16) >> 8;
    if modifiers & !1 != 0 {
        return None;
    }
    Some(Key {
        vk: (found as u16) & 0xFF,
        scan: 0,
        shift: modifiers & 1 != 0,
        caps: false,
    })
}

/// Retypes `text` as if its keys had been pressed in `to` instead of `from`. Characters
/// without a counterpart (digits, spaces, symbols missing from either layout) are kept.
pub fn convert(text: &str, from: Layout, to: Layout) -> String {
    text.chars()
        .map(|character| {
            key_for(character, from)
                .and_then(|key| key_text(key, to))
                .filter(|converted| converted.chars().count() == 1)
                .unwrap_or_else(|| character.to_string())
        })
        .collect()
}

/// Which layout of the pair `text` was most likely typed in: the one more of its characters
/// belong to exclusively.
pub fn source_of(text: &str, pair: Pair) -> Layout {
    let (mut english, mut other) = (0, 0);
    for character in text.chars() {
        let in_english = key_for(character, pair.english).is_some();
        let in_other = key_for(character, pair.other).is_some();
        match (in_english, in_other) {
            (true, false) => english += 1,
            (false, true) => other += 1,
            _ => {}
        }
    }
    if other > english {
        pair.other
    } else {
        pair.english
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The installed layout for a language, if this machine has it.
    fn layout(language: u16) -> Option<Layout> {
        installed()
            .into_iter()
            .find(|layout| layout.language() == language)
    }

    fn pairs() -> Vec<(Pair, &'static str, &'static str)> {
        let Some(english) = layout(ENGLISH) else {
            return Vec::new();
        };
        [
            // The comma key is `б`, so a typed comma becomes `б`: only shared keys survive.
            (0x19, "ghbdtn vbh!", "привет мир!"),
            (0x22, "ghbdsn cdsn!", "привіт світ!"),
        ]
        .into_iter()
        .filter_map(|(language, typed, meant)| {
            let other = layout(language)?;
            Some((Pair { english, other }, typed, meant))
        })
        .collect()
    }

    #[test]
    fn selected_text_converts_both_ways_and_detects_its_layout() {
        let pairs = pairs();
        if pairs.is_empty() {
            eprintln!("skipped: needs English with Russian or Ukrainian installed");
        }
        for (pair, typed, meant) in pairs {
            assert_eq!(convert(typed, pair.english, pair.other), meant);
            assert_eq!(convert(meant, pair.other, pair.english), typed);
            assert_eq!(source_of(typed, pair), pair.english);
            assert_eq!(source_of(meant, pair), pair.other);
            // Capitals come from Shift, which the conversion carries over.
            assert_eq!(
                convert("Ghbdtn", pair.english, pair.other).chars().next(),
                meant
                    .chars()
                    .next()
                    .map(|c| c.to_uppercase().next().unwrap())
            );
        }
    }

    #[test]
    fn pairing_prefers_the_current_layout_then_ukrainian() {
        let layout = |id: usize| Layout(HKL(id as *mut _));
        let (english, russian, ukrainian) = (layout(0x0409), layout(0x0419), layout(0x0422));
        let all = [english, russian, ukrainian];
        assert_eq!(pair(&all, russian).unwrap().other, russian);
        assert_eq!(pair(&all, english).unwrap().other, ukrainian);
        assert_eq!(pair(&all, english).unwrap().opposite(english), ukrainian);
        assert_eq!(pair(&all, ukrainian).unwrap().opposite(ukrainian), english);
        assert_eq!(pair(&[russian, ukrainian], russian), None);
    }
}

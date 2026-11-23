//! The fixes themselves, on their own thread so the hook never waits on the clipboard or a
//! spell checker.

use crate::auto::{self, Side, Speller};
use crate::buffer::{Key, LastWord};
use crate::hook::{Command, TYPED};
use crate::{input, layouts};
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::thread::sleep;
use std::time::{Duration, Instant};
use windows::Win32::{
    System::{
        Com::{COINIT_MULTITHREADED, CoInitializeEx},
        DataExchange::GetClipboardSequenceNumber,
    },
    UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_A, VK_C, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_V,
    },
};

/// An undo this soon after an automatic fix means the fix was wrong.
const UNDO_WINDOW: Duration = Duration::from_secs(15);
/// Remembered "do not fix" words; bounded so a long session cannot grow it without limit.
const MAX_EXCEPTIONS: usize = 1000;

pub fn run(commands: Receiver<Command>) {
    // SAFETY: Initializes COM for this thread, which owns the spell checkers.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let mut speller = Speller::new();
    let mut memory = Memory::default();
    for command in commands {
        match command {
            Command::FixWord(last) => {
                memory.learn_from_undo(&last.word);
                fix_word(&last);
            }
            Command::ConvertSelection => convert_selection(false),
            Command::ConvertAll => convert_selection(true),
            Command::WordEnded { last, stamp } => {
                if let Some(speller) = speller.as_mut()
                    && !memory.exceptions.contains(&last.word)
                    && auto_fix(speller, &last, stamp)
                {
                    memory.last_fix = Some((last.word, Instant::now()));
                }
            }
        }
    }
}

/// What automatic mode learned this session.
#[derive(Default)]
struct Memory {
    last_fix: Option<(Vec<Key>, Instant)>,
    exceptions: HashSet<Vec<Key>>,
}

impl Memory {
    /// Undoing an automatic fix (same keys, soon after) marks those keys as "leave alone".
    fn learn_from_undo(&mut self, word: &[Key]) {
        if let Some((fixed, at)) = self.last_fix.take()
            && fixed == word
            && at.elapsed() <= UNDO_WINDOW
            && self.exceptions.len() < MAX_EXCEPTIONS
        {
            self.exceptions.insert(fixed);
        }
    }
}

/// Fixes a just-finished word when it is not a word as typed but is one in the other layout.
/// Returns whether it did.
fn auto_fix(speller: &mut Speller, last: &LastWord, stamp: u64) -> bool {
    let (window, current) = layouts::foreground();
    let Some(pair) = layouts::pair(&layouts::installed(), current) else {
        return false;
    };
    let target = pair.opposite(current);
    let (Some(typed_tag), Some(other_tag)) =
        (auto::language_tag(current), auto::language_tag(target))
    else {
        return false;
    };
    let (Some(shown), Some(meant)) = (
        layouts::keys_text(&last.word, current),
        layouts::keys_text(&last.word, target),
    ) else {
        return false;
    };
    let fix = auto::should_fix(&shown, &meant, |side, word| match side {
        Side::Typed => speller.is_word(&typed_tag, word),
        Side::Other => speller.is_word(&other_tag, word),
    });
    // Anything typed or clicked since the space would be erased by the backspaces below.
    if !fix || TYPED.load(Ordering::Relaxed) != stamp {
        return false;
    }
    let Some(space) = layouts::keys_text(&last.trailing, target) else {
        return false;
    };
    input::backspaces(shown.chars().count() + last.trailing.len());
    input::type_text(&(meant + &space));
    layouts::switch(window, target);
    true
}

/// Erases the last word (and the spaces after it) and retypes the same keys in the other layout.
/// Pressing the hotkey again converts it back, since the recorded keys are unchanged.
fn fix_word(last: &LastWord) {
    let (window, current) = layouts::foreground();
    let Some(pair) = layouts::pair(&layouts::installed(), current) else {
        return;
    };
    let target = pair.opposite(current);
    let keys: Vec<_> = last.word.iter().chain(&last.trailing).copied().collect();
    let (Some(shown), Some(meant)) = (
        layouts::keys_text(&keys, current),
        layouts::keys_text(&keys, target),
    ) else {
        return;
    };
    input::backspaces(shown.chars().count());
    input::type_text(&meant);
    layouts::switch(window, target);
}

/// Copies the selection (or, with `select_all`, everything in the field), converts it to the
/// other layout and pastes it back. The previous clipboard text is restored afterwards; other
/// clipboard formats are not.
fn convert_selection(select_all: bool) {
    // The hotkey's own Ctrl may still be held; it would turn Ctrl+A into Ctrl+Shift+A and so on.
    wait_for_modifiers_release(Duration::from_secs(2));
    if select_all {
        input::chord(VK_CONTROL, VK_A);
        sleep(Duration::from_millis(30));
    }
    let (window, current) = layouts::foreground();
    let Some(pair) = layouts::pair(&layouts::installed(), current) else {
        return;
    };
    let saved = clipboard_win::get_clipboard_string().ok();
    // SAFETY: Plain query.
    let sequence = unsafe { GetClipboardSequenceNumber() };
    input::chord(VK_CONTROL, VK_C);
    if !wait_until(Duration::from_millis(500), || {
        // SAFETY: Plain query.
        unsafe { GetClipboardSequenceNumber() != sequence }
    }) {
        return; // Nothing was selected, or the window ignores Ctrl+C.
    }
    let Ok(text) = clipboard_win::get_clipboard_string() else {
        return;
    };
    let source = layouts::source_of(&text, pair);
    let target = pair.opposite(source);
    let converted = layouts::convert(&text, source, target);
    if converted != text && clipboard_win::set_clipboard_string(&converted).is_ok() {
        input::chord(VK_CONTROL, VK_V);
        // Let the window read the clipboard before the original text returns.
        sleep(Duration::from_millis(300));
        layouts::switch(window, target);
    }
    if let Some(saved) = saved {
        let _ = clipboard_win::set_clipboard_string(&saved);
    }
}

/// Shift+Pause leaves Shift held; Ctrl+C would become Ctrl+Shift+C.
fn wait_for_modifiers_release(limit: Duration) {
    wait_until(limit, || {
        [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .into_iter()
            // SAFETY: Plain query.
            .all(|vk| unsafe { GetAsyncKeyState(i32::from(vk.0)) } as u16 & 0x8000 == 0)
    });
}

fn wait_until(limit: Duration, mut ready: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + limit;
    while !ready() {
        if Instant::now() >= deadline {
            return false;
        }
        sleep(Duration::from_millis(10));
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(vk: u16) -> Vec<Key> {
        vec![Key {
            vk,
            scan: 0,
            shift: false,
            caps: false,
        }]
    }

    #[test]
    fn undoing_an_automatic_fix_soon_after_remembers_the_word_once() {
        // A manual fix of another word teaches nothing.
        let mut memory = Memory {
            last_fix: Some((word(1), Instant::now())),
            ..Default::default()
        };
        memory.learn_from_undo(&word(2));
        assert!(memory.exceptions.is_empty());
        // Undoing the same word right away does, and only once.
        memory.last_fix = Some((word(1), Instant::now()));
        memory.learn_from_undo(&word(1));
        assert!(memory.exceptions.contains(&word(1)));
        assert!(memory.last_fix.is_none());
        // A late undo is just a manual fix.
        memory.last_fix = Some((word(3), Instant::now() - UNDO_WINDOW * 2));
        memory.learn_from_undo(&word(3));
        assert!(!memory.exceptions.contains(&word(3)));
    }
}

//! Low-level keyboard and mouse hooks: record what is typed and catch the hotkeys.
//!
//! The callbacks run on the thread that installed them, inside its message loop, and must
//! return quickly or Windows drops the hook. They only update the buffer and hand work to
//! the action thread.

use crate::buffer::{Buffer, Key, Kind, LastWord};
use crate::input::MARKER;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};
use windows::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, GetKeyState, VIRTUAL_KEY, VK_BACK, VK_CAPITAL, VK_CONTROL,
            VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_NUMLOCK, VK_PAUSE, VK_RCONTROL,
            VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SCROLL, VK_SHIFT, VK_SPACE,
        },
        WindowsAndMessaging::{
            CallNextHookEx, GWL_STYLE, GetDesktopWindow, GetForegroundWindow, GetShellWindow,
            GetWindowLongW, GetWindowRect, HHOOK, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT,
            SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN,
            WM_KEYUP, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_SYSKEYDOWN, WM_SYSKEYUP,
            WM_XBUTTONDOWN, WS_CAPTION,
        },
    },
};

/// Work for the action thread.
pub enum Command {
    FixWord(LastWord),
    ConvertSelection,
    /// Select everything in the focused field first, then convert it.
    ConvertAll,
    /// A word was just followed by a space: automatic mode may fix it. `stamp` is
    /// [`TYPED`] at that moment; the fix is dropped if more was typed since.
    WordEnded {
        last: LastWord,
        stamp: u64,
    },
}

/// Set from the tray: while true, nothing is recorded and no hotkey fires.
pub static PAUSED: AtomicBool = AtomicBool::new(false);

/// Automatic mode: check every finished word. Set from the tray and saved settings.
pub static AUTO: AtomicBool = AtomicBool::new(true);

/// Counts text-changing key presses and clicks. The action thread compares it before
/// erasing anything, so a fix never deletes what the person typed after the word.
pub static TYPED: AtomicU64 = AtomicU64::new(0);

const HISTORY: usize = 256;

struct State {
    buffer: Buffer,
    foreground: isize,
    shift_taps: DoubleTap,
    ctrl_taps: DoubleTap,
    commands: Sender<Command>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Installed hooks; removed on drop.
pub struct Hooks([HHOOK; 2]);

impl Drop for Hooks {
    fn drop(&mut self) {
        for hook in self.0 {
            // SAFETY: Both handles came from SetWindowsHookExW on this thread.
            unsafe {
                let _ = UnhookWindowsHookEx(hook);
            }
        }
    }
}

/// Installs the hooks on the calling thread, which must then run a message loop.
pub fn install(commands: Sender<Command>) -> windows::core::Result<Hooks> {
    STATE.with(|state| {
        *state.borrow_mut() = Some(State {
            buffer: Buffer::new(HISTORY),
            foreground: 0,
            shift_taps: DoubleTap::default(),
            ctrl_taps: DoubleTap::default(),
            commands,
        });
    });
    // SAFETY: The callbacks are 'static functions; the module handle is this executable.
    unsafe {
        let module = HINSTANCE(GetModuleHandleW(None)?.0);
        let keyboard = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), Some(module), 0)?;
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), Some(module), 0)?;
        Ok(Hooks([keyboard, mouse]))
    }
}

fn pressed(vk: VIRTUAL_KEY) -> bool {
    // SAFETY: Plain state query.
    unsafe { GetAsyncKeyState(i32::from(vk.0)) as u16 & 0x8000 != 0 }
}

unsafe extern "system" fn keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && !PAUSED.load(Ordering::Relaxed) {
        // SAFETY: For WH_KEYBOARD_LL, lparam points to a KBDLLHOOKSTRUCT for this call.
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let message = wparam.0 as u32;
        let down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
        let up = message == WM_KEYUP || message == WM_SYSKEYUP;
        if info.dwExtraInfo != MARKER && (down || up) && on_key(info, down) {
            return LRESULT(1);
        }
    }
    // SAFETY: Forwarding the unchanged arguments.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let message = wparam.0 as u32;
        // SAFETY: For WH_MOUSE_LL, lparam points to an MSLLHOOKSTRUCT for this call.
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let click = [
            WM_LBUTTONDOWN,
            WM_RBUTTONDOWN,
            WM_MBUTTONDOWN,
            WM_XBUTTONDOWN,
        ];
        if info.dwExtraInfo != MARKER && click.contains(&message) {
            TYPED.fetch_add(1, Ordering::Relaxed);
            with_state(|state| state.buffer.clear());
        }
    }
    // SAFETY: Forwarding the unchanged arguments.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn with_state<T>(work: impl FnOnce(&mut State) -> T) -> Option<T> {
    STATE.with(|state| state.borrow_mut().as_mut().map(work))
}

/// Handles one key event; `true` swallows it.
fn on_key(info: &KBDLLHOOKSTRUCT, down: bool) -> bool {
    let vk = VIRTUAL_KEY(info.vkCode as u16);
    with_state(|state| {
        // SAFETY: Plain query.
        let window = unsafe { GetForegroundWindow() };
        let foreground = window.0 as isize;
        if foreground != state.foreground {
            state.foreground = foreground;
            state.buffer.clear();
        }
        // Games run full screen and use Shift for sprinting: never type into them.
        if fullscreen(window) {
            state.buffer.clear();
            state.shift_taps = DoubleTap::default();
            state.ctrl_taps = DoubleTap::default();
            return false;
        }
        let shift = pressed(VK_SHIFT);
        // Every key, Pause included, takes part in tap detection: Shift+Pause is not a tap.
        // Double Shift fixes the last word, double Ctrl the selection: for keyboards
        // without Pause, and free of the text-editing shortcuts such as Shift+End.
        let now = Instant::now();
        let is_shift = [VK_SHIFT, VK_LSHIFT, VK_RSHIFT].contains(&vk);
        let is_ctrl = [VK_CONTROL, VK_LCONTROL, VK_RCONTROL].contains(&vk);
        // Ctrl stays out of Shift tap detection: held Ctrl auto-repeats, and Ctrl+Shift,Shift
        // is its own hotkey (convert the whole field).
        if !is_ctrl && state.shift_taps.event(is_shift, down, now) {
            let command = if [VK_MENU, VK_LWIN, VK_RWIN].into_iter().any(pressed) {
                None // Alt+Shift twice is switching the layout, not a request to fix.
            } else if pressed(VK_CONTROL) {
                Some(Command::ConvertAll)
            } else {
                state.buffer.last_word().map(Command::FixWord)
            };
            if let Some(command) = command {
                let _ = state.commands.send(command);
            }
        }
        if state.ctrl_taps.event(is_ctrl, down, now) {
            let _ = state.commands.send(Command::ConvertSelection);
        }
        if vk == VK_PAUSE {
            if down {
                let command = if shift {
                    Some(Command::ConvertSelection)
                } else {
                    state.buffer.last_word().map(Command::FixWord)
                };
                if let Some(command) = command {
                    let _ = state.commands.send(command);
                }
            }
            return true;
        }
        if down {
            let chord = [VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
                .into_iter()
                .any(pressed);
            let kind = classify(vk, chord);
            let key = Key {
                vk: vk.0,
                scan: info.scanCode as u16,
                shift,
                // SAFETY: Plain query; the low bit is the toggle state.
                caps: unsafe { GetKeyState(i32::from(VK_CAPITAL.0)) } & 1 != 0,
            };
            state.buffer.push(kind, key);
            if kind != Kind::Ignore {
                let stamp = TYPED.fetch_add(1, Ordering::Relaxed) + 1;
                // The first space after a word ends it; further spaces do not re-check it.
                if kind == Kind::Separator
                    && AUTO.load(Ordering::Relaxed)
                    && let Some(last) = state.buffer.last_word()
                    && last.trailing.len() == 1
                {
                    let _ = state.commands.send(Command::WordEnded { last, stamp });
                }
            }
        }
        false
    })
    .unwrap_or(false)
}

/// Whether the window covers its whole monitor (games, full-screen video).
fn fullscreen(window: HWND) -> bool {
    // SAFETY: Plain queries into initialized locals; cbSize is set as the API requires.
    unsafe {
        let mut rect = RECT::default();
        if window.is_invalid() || GetWindowRect(window, &mut rect).is_err() {
            return false;
        }
        let monitor = MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return false;
        }
        let screen = info.rcMonitor;
        // A maximized window with an auto-hidden taskbar also covers the monitor but keeps
        // its title bar; full-screen games and players drop it.
        let caption = GetWindowLongW(window, GWL_STYLE) as u32 & WS_CAPTION.0 == WS_CAPTION.0;
        !caption
            && rect.left <= screen.left
            && rect.top <= screen.top
            && rect.right >= screen.right
            && rect.bottom >= screen.bottom
            && GetShellWindow() != window
            && GetDesktopWindow() != window
    }
}

/// How a key press affects the typed text. `chord` is true while Ctrl, Alt or Win is held.
fn classify(vk: VIRTUAL_KEY, chord: bool) -> Kind {
    let modifiers = [
        VK_SHIFT,
        VK_LSHIFT,
        VK_RSHIFT,
        VK_CONTROL,
        VK_LCONTROL,
        VK_RCONTROL,
        VK_MENU,
        VK_LMENU,
        VK_RMENU,
        VK_LWIN,
        VK_RWIN,
        VK_CAPITAL,
        VK_NUMLOCK,
        VK_SCROLL,
    ];
    if modifiers.contains(&vk) {
        return Kind::Ignore;
    }
    if chord {
        // Shortcuts (Ctrl+V, Alt+Tab, …) change text or focus in ways we cannot follow.
        return Kind::Reset;
    }
    match vk.0 {
        _ if vk == VK_BACK => Kind::Backspace,
        _ if vk == VK_SPACE => Kind::Separator,
        // Digits and letters.
        0x30..=0x39 | 0x41..=0x5A => Kind::Word,
        // Punctuation keys: letters in Cyrillic layouts (`;` is `ж`, `,` is `б`, …).
        0xBA..=0xC0 | 0xDB..=0xDE | 0xE2 => Kind::Word,
        _ => Kind::Reset,
    }
}

/// Two quick taps of one modifier with nothing in between.
#[derive(Default)]
struct DoubleTap {
    down_at: Option<Instant>,
    clean: bool,
    last_tap: Option<Instant>,
}

const TAP: Duration = Duration::from_millis(300);
const BETWEEN_TAPS: Duration = Duration::from_millis(400);

impl DoubleTap {
    /// Feeds one key event; `true` when it completes a double tap.
    fn event(&mut self, is_target: bool, down: bool, now: Instant) -> bool {
        if !is_target {
            if down {
                self.clean = false;
                self.last_tap = None;
            }
            return false;
        }
        if down {
            // Auto-repeat sends more downs while held; only the first starts a tap.
            if self.down_at.is_none() {
                self.down_at = Some(now);
                self.clean = true;
            }
            return false;
        }
        let tapped = self.clean
            && self
                .down_at
                .take()
                .is_some_and(|at| now.duration_since(at) <= TAP);
        self.down_at = None;
        if !tapped {
            self.last_tap = None;
            return false;
        }
        if self
            .last_tap
            .take()
            .is_some_and(|at| now.duration_since(at) <= BETWEEN_TAPS)
        {
            return true;
        }
        self.last_tap = Some(now);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punctuation_keys_are_word_keys_and_shortcuts_reset() {
        assert_eq!(classify(VIRTUAL_KEY(0xBC), false), Kind::Word); // `,` is `б`
        assert_eq!(classify(VIRTUAL_KEY(0xDE), false), Kind::Word); // `'` is `э`/`є`
        assert_eq!(classify(VIRTUAL_KEY(0x56), true), Kind::Reset); // Ctrl+V
        assert_eq!(classify(VIRTUAL_KEY(0x25), false), Kind::Reset); // Left arrow
        assert_eq!(classify(VK_LSHIFT, false), Kind::Ignore);
    }

    #[test]
    fn double_shift_tap_fires_only_without_keys_between_and_within_time() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let mut taps = DoubleTap::default();
        // Shift+H while typing a capital is not a tap.
        assert!(!taps.event(true, true, at(0)));
        assert!(!taps.event(false, true, at(20)));
        assert!(!taps.event(true, false, at(60)));
        // Two clean taps fire once.
        assert!(!taps.event(true, true, at(1000)));
        assert!(!taps.event(true, false, at(1050)));
        assert!(!taps.event(true, true, at(1150)));
        assert!(!taps.event(true, true, at(1160))); // auto-repeat
        assert!(taps.event(true, false, at(1200)));
        // A third tap starts over instead of firing again.
        assert!(!taps.event(true, true, at(1300)));
        assert!(!taps.event(true, false, at(1350)));
        // Taps too far apart do not fire.
        assert!(!taps.event(true, true, at(3000)));
        assert!(!taps.event(true, false, at(3050)));
        assert!(!taps.event(true, true, at(3600)));
        assert!(!taps.event(true, false, at(3650)));
    }
}

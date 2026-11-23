//! Synthetic keyboard input. Every event carries [`MARKER`] so the hook ignores our own typing.

use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, SendInput, VIRTUAL_KEY, VK_BACK,
};

/// `dwExtraInfo` of events this program injects ("ROZK").
pub const MARKER: usize = 0x524F_5A4B;

fn event(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MARKER,
            },
        },
    }
}

fn send(events: &[INPUT]) {
    // SAFETY: The slice holds fully initialized keyboard inputs of the declared size.
    unsafe {
        SendInput(events, std::mem::size_of::<INPUT>() as i32);
    }
}

fn tap(vk: VIRTUAL_KEY) -> [INPUT; 2] {
    [
        event(vk, 0, KEYBD_EVENT_FLAGS(0)),
        event(vk, 0, KEYEVENTF_KEYUP),
    ]
}

pub fn backspaces(count: usize) {
    let events: Vec<INPUT> = (0..count).flat_map(|_| tap(VK_BACK)).collect();
    send(&events);
}

/// Types text as Unicode characters, independent of the window's current layout.
pub fn type_text(text: &str) {
    let events: Vec<INPUT> = text
        .encode_utf16()
        .flat_map(|unit| {
            [
                event(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE),
                event(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP),
            ]
        })
        .collect();
    send(&events);
}

/// Presses `key` while holding `modifier`, e.g. Ctrl+C.
pub fn chord(modifier: VIRTUAL_KEY, key: VIRTUAL_KEY) {
    let [key_down, key_up] = tap(key);
    send(&[
        event(modifier, 0, KEYBD_EVENT_FLAGS(0)),
        key_down,
        key_up,
        event(modifier, 0, KEYEVENTF_KEYUP),
    ]);
}

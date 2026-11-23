//! Rozkladka: fixes text typed in the wrong keyboard layout.
//!
//! Pause (or Shift twice) retypes the last word in the other layout; Ctrl twice (or
//! Shift+Pause) converts the selection. Lives in the tray.
#![cfg_attr(not(test), windows_subsystem = "windows")]

mod actions;
mod auto;
mod autostart;
mod buffer;
mod hook;
mod input;
mod layouts;
mod settings;

use std::sync::atomic::Ordering;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};
use windows::Win32::{
    Foundation::{ERROR_ALREADY_EXISTS, GetLastError, LPARAM, WPARAM},
    System::Threading::{CreateMutexW, GetCurrentThreadId},
    UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, PostThreadMessageW, TranslateMessage, WM_QUIT,
    },
};
use windows::core::w;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // One instance per session: two hooks would fix every word twice.
    // SAFETY: A named mutex held for the life of the process.
    let _instance = unsafe { CreateMutexW(None, true, w!("Local\\Rozkladka"))? };
    // SAFETY: Reads the error of the call above.
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return Ok(());
    }

    let (commands, work) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("actions".into())
        .spawn(move || actions::run(work))?;
    let _hooks = hook::install(commands)?;

    let menu = Menu::new();
    let title = MenuItem::new(
        concat!("Rozkladka ", env!("CARGO_PKG_VERSION")),
        false,
        None,
    );
    let fix_word = MenuItem::new("Fix last word: Pause or Shift twice", false, None);
    let fix_selection = MenuItem::new("Fix selection: Ctrl twice or Shift+Pause", false, None);
    let fix_all = MenuItem::new("Fix whole field: hold Ctrl, Shift twice", false, None);
    hook::AUTO.store(settings::auto(), Ordering::Relaxed);
    let auto = CheckMenuItem::new(
        "Fix automatically after Space",
        true,
        settings::auto(),
        None,
    );
    let paused = CheckMenuItem::new("Pause", true, false, None);
    let startup = CheckMenuItem::new("Start with Windows", true, autostart::enabled(), None);
    let quit = MenuItem::new("Quit", true, None);
    menu.append_items(&[
        &title,
        &PredefinedMenuItem::separator(),
        &fix_word,
        &fix_selection,
        &fix_all,
        &PredefinedMenuItem::separator(),
        &auto,
        &paused,
        &startup,
        &quit,
    ])?;
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Rozkladka: Pause fixes the last word")
        .with_icon(icon()?)
        .build()?;

    // SAFETY: Plain query.
    let main_thread = unsafe { GetCurrentThreadId() };
    let (auto_id, paused_id, startup_id, quit_id) = (
        auto.id().clone(),
        paused.id().clone(),
        startup.id().clone(),
        quit.id().clone(),
    );
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == auto_id {
            let enabled = !hook::AUTO.fetch_xor(true, Ordering::Relaxed);
            settings::set_auto(enabled);
        } else if event.id == paused_id {
            hook::PAUSED.fetch_xor(true, Ordering::Relaxed);
        } else if event.id == startup_id {
            autostart::set(!autostart::enabled());
        } else if event.id == quit_id {
            // SAFETY: Ends the main thread's message loop.
            unsafe {
                let _ = PostThreadMessageW(main_thread, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }));

    // The hooks and the tray icon are serviced by this loop.
    let mut message = MSG::default();
    // SAFETY: Standard message loop on the thread that owns the hooks and the tray window.
    unsafe {
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

/// A blue rounded square with two opposite arrows: "switch layout".
fn icon() -> Result<Icon, tray_icon::BadIcon> {
    const SIZE: usize = 32;
    let mut rgba = vec![0_u8; SIZE * SIZE * 4];
    let mut paint = |x: usize, y: usize, color: [u8; 4]| {
        let at = (y * SIZE + x) * 4;
        rgba[at..at + 4].copy_from_slice(&color);
    };
    let (blue, white) = ([47, 111, 235, 255], [255, 255, 255, 255]);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let corner = |c: usize| c.min(SIZE - 1 - c);
            let (cx, cy) = (corner(x), corner(y));
            // Round the corners with a 5 px radius.
            if cx < 5 && cy < 5 && (5 - cx).pow(2) + (5 - cy).pow(2) > 26 {
                continue;
            }
            paint(x, y, blue);
        }
    }
    for x in 7..25 {
        for y in [10, 11, 20, 21] {
            paint(x, y, white);
        }
    }
    for step in 0..5 {
        // Right-pointing head on the top arrow, left-pointing on the bottom one.
        for y in (10 - step)..=(11 + step) {
            paint(24 - step, y, white);
            paint(7 + step, y + 10, white);
        }
    }
    Icon::from_rgba(rgba, SIZE as u32, SIZE as u32)
}

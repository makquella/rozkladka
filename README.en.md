# Rozkladka

[Українською](README.md) · **English**

**Typed `ghbdtn` instead of `привет`? Press Space and it fixes itself.**

![Rozkladka: dljvf becomes «вдома» because the layout was wrong](assets/demo.gif)

A tiny open-source alternative to Punto Switcher for Windows: no ads, no telemetry, no account,
one small executable that lives in the tray.

## Download

Get `rozkladka.exe` from [the latest release](https://github.com/makquella/rozkladka/releases/latest)
and run it: no installer, it appears in the tray. *Start with Windows* is in the tray menu.

## Automatic mode

After every word, when you press Space, Rozkladka checks it: if the word is **not** a word in
the layout you typed it in, but the same keys spell a **real word** in the other layout, it is
retyped and the layout switches. `ghbdtn ` becomes `привет `, `руддщ ` becomes `hello `.

The dictionaries are Windows' own spell checkers (Settings → Time & language → Language →
your language → Options → *Basic typing*). Nothing is bundled, every word form is known, and
words you added to the Windows dictionary count.

Wrong fix? Tap **Shift** twice right away: the word goes back, and Rozkladka leaves that word
alone for the rest of the session. Single letters are never fixed automatically, and anything
you type or click after the space cancels a pending fix. Toggle the mode in the tray menu.

## Hotkeys

| Keys | What happens |
| --- | --- |
| **Pause** or **Shift, Shift** (two quick taps) | Retypes the last word (and the spaces after it) in the other layout and switches the layout. Press again to undo. |
| **Ctrl, Ctrl** (two quick taps) or **Shift+Pause** | Converts the selected text to the other layout. |
| Hold **Ctrl**, tap **Shift, Shift** | Selects everything in the field (Ctrl+A) and converts it. |

It works with any layouts installed in Windows: English with Ukrainian, Russian, Belarusian
and others. Characters come from Windows' own layout tables, so nothing is hard-coded.

## Privacy

Rozkladka has to see your key presses to know what the last word was. It keeps **only the
last 256 keys in memory**, forgets them on every click, shortcut or window switch, never writes
them to disk and has no network code at all. The source is short: read `src/hook.rs`.

It does nothing in full-screen windows, so games never receive synthetic typing.

## Limits (0.1)

- Windows only.
- Automatic mode needs Windows spelling for both languages (see above); without it, use the
  hotkeys. Learned exceptions are forgotten on restart.
- Windows running as administrator ignore input from a normal program; run Rozkladka as
  administrator too if you need it there.
- Converting a selection goes through the clipboard: the previous clipboard **text** is
  restored, other clipboard content (images, files) is not.
- The executable is not code-signed yet, so SmartScreen may warn on first start.

## How it works

```
 keyboard/mouse hook (main thread)          action thread
 ┌──────────────────────────────┐   cmd   ┌────────────────────────────────────┐
 │ record physical keys of the  │ ──────▶ │ last word: Backspace × n, type the │
 │ current word (buffer.rs);    │         │ same keys in the other layout as   │
 │ reset on click / shortcut /  │         │ Unicode, switch layout             │
 │ window change; detect Pause, │         │ selection: Ctrl+C, convert, Ctrl+V,│
 │ double Shift/Ctrl (hook.rs)  │         │ restore clipboard (actions.rs)     │
 └──────────────────────────────┘         │ after Space: spell-check (auto.rs) │
                                          └────────────────────────────────────┘
                 layouts.rs: ToUnicodeEx / VkKeyScanExW conversion between layouts
```

The hook stores **keys**, not characters: the key that typed `g` types `п` in the other
layout. Conversion asks Windows what each recorded key produces in the target layout, so any
installed pair works, Shift and Caps Lock included.

## Roadmap

1. Persistent exceptions, per-app opt-out, fixing after punctuation and Enter, not just Space.
2. Settings window: hotkeys, layout pair, apps to ignore.
3. Signed installer, `winget install rozkladka`.
4. Ukrainian, English and Russian interface.

## Build

```powershell
cargo build --release
```

The result is `target\release\rozkladka.exe`. Rust 1.88 or newer (edition 2024).

## License

MIT or Apache-2.0, at your option: see `LICENSE-MIT` and `LICENSE-APACHE`.

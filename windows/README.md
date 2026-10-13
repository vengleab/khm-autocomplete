# Windows text service

A Text Services Framework (TSF) text service for Khmer: Khmer typing (Khmer NiDA layout), Khmerlish and English → Khmer,
with a candidate window. Written in Rust with the `windows` crate. **Written and cross-checked on Linux: it has not been
built into a DLL or run on Windows** (see "What has not been verified").

```
 any app (Notepad, Word, a browser...)               one per user session
┌──────────────────────────────────────┐   named pipe   ┌────────────────────────────┐
│ khtsf.dll  (tsf/)                    │  \\.\pipe\...  │ khengine.exe  (engine/)    │
│  key sink, edit sessions, candidate  │ ─────────────► │  khdata.bin + khcore       │
│  window, Khmer layout table          │ ◄───────────── │  (language data, later the │
│  thin client: no language data       │   JSON frames  │   neural model)            │
└──────────────────────────────────────┘                └────────────────────────────┘
        khipc (ipc/): framing, client with deadlines, circuit breaker, server loop
```
| crate | what | Linux-testable |
|---|---|---|
| `ipc/` `khipc` | framed JSON protocol, `Client` with per-call deadlines, `Breaker`, `serve_connection`; Windows named-pipe transport (`winpipe`) | yes (Unix sockets), 8 tests |
| `engine/` `khengine` | `khengine.exe`: loads `khdata.bin`, serves the pipe (a Unix socket on other systems, for development) | yes, 5 tests incl. the real binary |
| `tsf/` `khtsf` | `khtsf.dll`: the text service. `logic.rs` (what each key means) is portable and tested | logic only, 5 tests; the rest is `cfg(windows)` |

## Crash-safety rules (the DLL runs inside every app)
1. **Tiny, and no language data.** The DLL links only `khlayout` (a 47-key table) and `khipc`. The vocabulary, bigrams and any model
   live in `khengine.exe`. If the engine is down, Khmer typing still works; Latin/English just show no suggestions.
2. **Never block the host.** Every engine call has a hard deadline (connect 20 ms, ping 30 ms, call 30 ms) enforced with overlapped I/O
   and `CancelIoEx`. After 2 failures in a row a circuit breaker stops calling for 3 s, so a dead engine costs the host app a
   couple of short stalls, not one per key. All timing constants are in `tsf/src/link.rs`.
3. **Fail open.** Any error, timeout or missing engine means the key passes through untouched. If Khmer mode cannot insert text
   (an app refuses the edit), the key is reported as *not eaten*, so it is never silently lost.
4. **No panic crosses a COM boundary.** Every COM method, DLL export and the window procedure runs inside `catch_unwind` and returns
   an error code instead. (`panic = "abort"` is never set for this crate.)
5. **No re-entrancy bugs.** No `RefCell` borrow is held across a call into TSF or the engine.
6. **Nothing typed is stored or logged.** Only the chosen mode is saved (`HKCU\Software\KhmerAutocomplete\Mode`).
7. **Passwords:** the text service is registered as a keyboard TIP *without* the secure-mode category, so Windows turns it off in
   password fields on its own. Read-only fields are also skipped.
8. **Engine start:** if the pipe is missing the DLL starts `khengine.exe` from its own folder (at most once per 10 s, detached, no
   window); the installer also starts it at logon. **Pipe squatting:** the pipe name is predictable, so on a shared machine another user could create it first and
   read the text before the caret. The engine therefore creates its first instance with `FILE_FLAG_FIRST_PIPE_INSTANCE` (and exits if the
   name is taken), and the DLL checks, before sending anything, that the server process runs as the same user
   (`GetNamedPipeServerProcessId` + `EqualSid`). The pipe is per user (`\\.\pipe\KhmerAutocomplete.<SID>`), refuses remote clients,
   and its security descriptor allows only the owner plus app-container and low-integrity processes (sandboxed apps).

## Build (on Windows, Rust + Visual Studio Build Tools)
```
windows\build.ps1            # x64 + x86 + arm64 DLLs, x64 engine, khdata.bin  ->  windows\dist\
```
or by hand: `cargo build --release -p khtsf --target x86_64-pc-windows-msvc` (also `i686-...` for 32-bit apps, `aarch64-...` for ARM64),
`cargo build --release -p khengine --target x86_64-pc-windows-msvc`, and
`khbuild data\data.json data\lexicon.json khdata.bin data\english.json`.
Cross-check from Linux (what was done here): `cargo check -p khtsf -p khengine -p khipc --target x86_64-pc-windows-msvc`.

## Install for testing (elevated prompt)
```
copy khtsf.dll, khengine.exe, khdata.bin into one folder, e.g. C:\Program Files\KhmerAutocomplete
regsvr32 "C:\Program Files\KhmerAutocomplete\khtsf.dll"          # registers COM class + Khmer (km-KH) profile
start "" "C:\Program Files\KhmerAutocomplete\khengine.exe"        # optional: the DLL starts it itself
```
Then Settings → Time & language → Language & region → Khmer → Language options → Keyboards: add "Khmer Autocomplete".
Remove with `regsvr32 /u khtsf.dll` (32-bit DLL: use `C:\Windows\SysWOW64\regsvr32.exe`). Installer: `installer/khmer.wxs` (WiX v4 template).

## Using it
- **Khmer script** (default): keys type Khmer by physical position (NiDA layout; AltGr = right Alt is the third level). Space is a
  normal space. Suggestions appear below the caret; **Tab** takes the first, or click one.
- **Latin (Khmerlish):** type `knhom`, **Space** converts it. **English → Khmer:** type `water`, **Space** converts it.
- **Ctrl + Alt + Space** cycles the mode and shows its name next to the caret.

## What has not been verified (read this before trusting anything here)
Everything that needs Windows. The code type-checks for x64, x86 and arm64 against `windows` 0.62.2, and the object file defines
`DllGetClassObject`, `DllCanUnloadNow`, `DllRegisterServer`, `DllUnregisterServer`, `DllMain`; nothing was linked or run.
Check these first, in order:
1. `regsvr32` succeeds, and the profile appears under Khmer keyboards (`RegisterProfile` with no icon file; add an icon).
2. The text service activates in Notepad. Typing letters in Khmer mode inserts Khmer (`ITfRange::SetText` via `TF_ES_SYNC` edit sessions).
3. `OnEndEdit` fires after a typed letter in Latin mode, and the candidate window appears at the caret (`ITfContextView::GetTextExt`).
   Coordinates and DPI in mixed-DPI setups are the most likely thing to be wrong.
4. Tab/Space/Enter replace the right span (`replace_len_utf16` counts UTF-16 units, which is what TSF ranges use).
5. Behavior in Chrome/Edge, Word, VS Code (Electron), Windows Terminal, and in a password field (must be off).
6. A killed `khengine.exe`: typing must stay smooth (breaker), and the engine must come back on its own.
Known gaps: no UI-less / immersive (UWP, Settings app) candidate UI (the TIP does not declare `TIPCAP_IMMERSIVESUPPORT`, so
Windows will not load it in such apps); no per-app disable list; no language-bar mode icon; processes that only load
Microsoft-signed code will not load this DLL.

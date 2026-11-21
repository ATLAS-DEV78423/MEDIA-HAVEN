//! Pure, name-only pre-download guard. qB writes no file bytes while a task is
//! paused, so rejecting a task here keeps executables off disk entirely.
//!
//! shortcut: name-based only — a deliberately renamed executable is caught by
//! the existing post-download probe/Defender gates, not here; upgrade only if
//! qB exposes byte inspection before download.

/// File extensions treated as executable, matched case-insensitively in the
/// final extension or any inner dotted segment of a declared file name.
pub const EXECUTABLE_EXTENSIONS: &[&str] = &[
    "exe", "com", "scr", "pif", "bat", "cmd", "msi", "msp", "jar", "js", "jse", "vbs", "vbe",
    "wsf", "wsh", "hta", "ps1", "ps1xml", "lnk", "reg", "dll", "ocx", "sys", "drv", "cpl", "msc",
    "app", "appx", "msix",
];

/// True when any declared file name carries a known executable extension,
/// either as its final extension or as an inner dotted segment. Both `/` and
/// `\` separators are accepted; matching is ASCII case-insensitive.
pub fn has_executable_file(names: &[String]) -> bool {
    names.iter().any(|name| {
        let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
        file.split('.').skip(1).any(|part| {
            let part = part.trim().to_ascii_lowercase();
            EXECUTABLE_EXTENSIONS.iter().any(|ext| *ext == part)
        })
    })
}

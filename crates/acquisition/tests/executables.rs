use media_acquisition::{has_executable_file, EXECUTABLE_EXTENSIONS};

#[test]
fn rejects_windows_executable_extensions_case_insensitively() {
    for name in [
        "movie.EXE",
        "tool.bat",
        "install.MSI",
        "payload.scr",
        "x.ps1",
        "a.dll",
    ] {
        assert!(has_executable_file(&[name.to_string()]), "{name}");
    }
    assert!(!EXECUTABLE_EXTENSIONS.is_empty());
}

#[test]
fn rejects_double_extension_and_windows_path_separators() {
    assert!(has_executable_file(&[r"show\S01E01.mp4.exe".to_string()]));
    assert!(has_executable_file(&["show/S01E01.exe.mkv".to_string()]));
}

#[test]
fn allows_ordinary_media_names() {
    let names = vec![
        "show/S01E01.mkv".to_string(),
        "movie.2024.1080p.mp4".to_string(),
        "subtitle.en.srt".to_string(),
        "cover.jpg".to_string(),
    ];
    assert!(!has_executable_file(&names));
    assert!(!has_executable_file(&[]));
}

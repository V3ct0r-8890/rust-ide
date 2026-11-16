//! The system clipboard from a terminal. Windows has its own clipboard API that
//! works in every console. Elsewhere, copying sends the OSC 52 escape sequence
//! (most terminals put the text on the clipboard) and also tries wl-copy /
//! xclip / xsel; pasting reads them, and the terminal's own paste arrives as a
//! bracketed paste event.

#[cfg(not(windows))]
use std::io::Write;

/// Puts text on the system clipboard (best effort).
pub fn write(text: &str) {
    #[cfg(windows)]
    {
        let _ = clipboard_win::set_clipboard_string(text);
    }
    #[cfg(not(windows))]
    {
        let mut out = std::io::stdout();
        let _ = write!(out, "\x1b]52;c;{}\x07", base64(text.as_bytes()));
        let _ = out.flush();
        for (program, args) in [("wl-copy", &[][..]), ("xclip", &["-selection", "clipboard"][..]), ("xsel", &["--clipboard", "--input"][..])] {
            if rust_ide::platform::find_program(program).is_some() && pipe_to(program, args, text) {
                break;
            }
        }
    }
}

/// The text on the system clipboard, if it can be read.
pub fn read() -> Option<String> {
    #[cfg(windows)]
    {
        clipboard_win::get_clipboard_string().ok()
    }
    #[cfg(not(windows))]
    {
        for (program, args) in [("wl-paste", &["--no-newline"][..]), ("xclip", &["-selection", "clipboard", "-o"][..]), ("xsel", &["--clipboard", "--output"][..])] {
            if rust_ide::platform::find_program(program).is_none() {
                continue;
            }
            if let Ok(output) = std::process::Command::new(program).args(args).stderr(std::process::Stdio::null()).output()
                && output.status.success()
            {
                return Some(String::from_utf8_lossy(&output.stdout).into_owned());
            }
        }
        None
    }
}

#[cfg(not(windows))]
fn pipe_to(program: &str, args: &[&str], text: &str) -> bool {
    use std::process::{Command, Stdio};
    let Ok(mut child) = Command::new(program).args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    // wl-copy and xclip stay in the background to serve the clipboard; do not wait for them.
    true
}

/// Standard base64, for OSC 52.
#[cfg_attr(windows, allow(dead_code))]
pub fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(super::base64(b""), "");
        assert_eq!(super::base64(b"f"), "Zg==");
        assert_eq!(super::base64(b"fo"), "Zm8=");
        assert_eq!(super::base64(b"foo"), "Zm9v");
        assert_eq!(super::base64("ไทย".as_bytes()), "4LmE4LiX4Lii");
    }
}

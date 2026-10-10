//! Delphi `TextFile` semantics used by the save/load and export code (see recon/RTL_SEMANTICS.md):
//! `Read(f, Integer)` skips blanks (chars <= ' ', across lines), reads a token of up to 32 chars
//! and converts it with Val; `ReadLn(f, s)` reads up to CR/LF; `ReadLn(f)` skips past the next LF;
//! `Writeln` emits CR LF. I/O errors raise EInOutError ({$I+}).

use super::mem::{Exc, R};

/// EInOutError message for an InOutRes code (SysUtils.InOutError / SysConst).
pub fn inout_message(code: i32) -> String {
    match code {
        2 => "File not found".into(),
        3 => "Invalid filename".into(),
        4 => "Too many open files".into(),
        5 => "File access denied".into(),
        100 => "Read beyond end of file".into(),
        101 => "Disk full".into(),
        106 => "Invalid numeric input".into(),
        _ => format!("I/O error {code}"),
    }
}

fn io_err(e: &std::io::Error) -> Exc {
    let code = match e.kind() {
        std::io::ErrorKind::NotFound => 2,
        std::io::ErrorKind::PermissionDenied => 5,
        _ => e.raw_os_error().unwrap_or(5),
    };
    Exc::Convert(inout_message(code))
}

pub struct TextReader {
    data: Vec<u8>,
    pos: usize,
}

impl TextReader {
    /// AssignFile + Reset.
    pub fn open(path: &str) -> R<TextReader> {
        let data = std::fs::read(path).map_err(|e| io_err(&e))?;
        Ok(TextReader { data, pos: 0 })
    }
    pub fn from_bytes(data: Vec<u8>) -> TextReader {
        TextReader { data, pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        // ^Z (EOF char) ends text input like the end of the file.
        self.data.get(self.pos).copied().filter(|&c| c != 0x1A)
    }

    /// ReadLn(f, s): characters up to (not including) CR or LF, then skip the line end.
    pub fn read_line(&mut self) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if c == b'\r' || c == b'\n' {
                break;
            }
            out.push(c as char); // Latin-1
            self.pos += 1;
        }
        self.read_ln();
        out
    }

    /// ReadLn(f) after a Read: skip to just past the next LF (a lone CR does not end the line).
    pub fn read_ln(&mut self) {
        while let Some(c) = self.peek() {
            self.pos += 1;
            if c == b'\n' {
                break;
            }
        }
    }

    fn token(&mut self, max: usize) -> Option<String> {
        while let Some(c) = self.peek() {
            if c > b' ' {
                break;
            }
            self.pos += 1;
        }
        self.peek()?;
        let mut t = String::new();
        while let Some(c) = self.peek() {
            if c <= b' ' || t.len() >= max {
                break;
            }
            t.push(c as char);
            self.pos += 1;
        }
        Some(t)
    }

    /// Read(f, Integer) — `_ReadLong` with `_ValLong` (decimal, optional sign, `$` hex).
    pub fn read_long(&mut self) -> R<i32> {
        let Some(t) = self.token(32) else { return Ok(0) };
        val_int(&t).and_then(|v| i32::try_from(v).ok()).ok_or_else(|| Exc::Convert(inout_message(106)))
    }

    /// Read(f, Int64) — `_ReadInt64`.
    pub fn read_int64(&mut self) -> R<i64> {
        let Some(t) = self.token(32) else { return Ok(0) };
        val_int(&t).ok_or_else(|| Exc::Convert(inout_message(106)))
    }
}

fn val_int(t: &str) -> Option<i64> {
    let (neg, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let v = if let Some(h) = rest.strip_prefix('$').or_else(|| rest.strip_prefix("0x")).or_else(|| rest.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()?
    } else if let Some(h) = rest.strip_prefix('x').or_else(|| rest.strip_prefix('X')) {
        i64::from_str_radix(h, 16).ok()?
    } else {
        if rest.is_empty() || !rest.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        rest.parse::<i64>().ok()?
    };
    Some(if neg { -v } else { v })
}

/// AssignFile + Rewrite; `Writeln` appends CR LF. Strings are written as Latin-1.
pub struct TextWriter {
    path: String,
    buf: Vec<u8>,
}

impl TextWriter {
    pub fn create(path: &str) -> R<TextWriter> {
        // Rewrite truncates/creates the file immediately.
        std::fs::write(path, b"").map_err(|e| io_err(&e))?;
        Ok(TextWriter { path: path.to_string(), buf: Vec::new() })
    }
    /// A writer that only collects bytes (no file).
    pub fn memory() -> TextWriter {
        TextWriter { path: String::new(), buf: Vec::new() }
    }
    pub fn write_bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
    pub fn write_str(&mut self, s: &str) {
        self.buf.extend(s.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }));
    }
    pub fn writeln(&mut self, s: &str) {
        self.write_str(s);
        self.buf.extend_from_slice(b"\r\n");
    }
    /// Write(f, Boolean)
    pub fn writeln_bool(&mut self, b: bool) {
        self.writeln(if b { "TRUE" } else { "FALSE" });
    }
    /// CloseFile.
    pub fn close(self) -> R<()> {
        std::fs::write(&self.path, &self.buf).map_err(|e| io_err(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delphi_read_semantics() {
        let mut r = TextReader::from_bytes(b"NAME\r\nTRUE\r\n  42 \r\n7\r\n$10\r\n".to_vec());
        assert_eq!(r.read_line(), "NAME");
        assert_eq!(r.read_line(), "TRUE");
        assert_eq!(r.read_long().unwrap(), 42);
        r.read_ln();
        assert_eq!(r.read_long().unwrap(), 7);
        r.read_ln();
        assert_eq!(r.read_long().unwrap(), 16);
        r.read_ln();
        assert_eq!(r.read_long().unwrap(), 0); // EOF -> 0
    }
}

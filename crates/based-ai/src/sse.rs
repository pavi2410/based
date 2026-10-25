//! Incremental Server-Sent Events parser (pure, no I/O).

use std::mem::take;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

#[derive(Debug, Default)]
pub struct SseParser {
    buf: Vec<u8>,
    event: Option<String>,
    data: Vec<String>,
}

impl SseParser {
    /// Feeds raw bytes and returns every event completed by them.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<SseEvent> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let raw: Vec<u8> = self.buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&raw);
            let line = line.trim_end_matches(['\n', '\r']);
            if let Some(ev) = self.line(line) {
                out.push(ev);
            }
        }
        out
    }

    fn line(&mut self, line: &str) -> Option<SseEvent> {
        if line.is_empty() {
            if self.data.is_empty() && self.event.is_none() {
                return None;
            }
            return Some(SseEvent {
                event: self.event.take(),
                data: take(&mut self.data).join("\n"),
            });
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => self.event = Some(value.to_owned()),
            "data" => self.data.push(value.to_owned()),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(event: Option<&str>, data: &str) -> SseEvent {
        SseEvent {
            event: event.map(Into::into),
            data: data.into(),
        }
    }

    #[test]
    fn parses_named_and_unnamed_events() {
        let mut p = SseParser::default();
        let got = p.feed(b"event: a\ndata: 1\n\ndata: 2\n\n");
        assert_eq!(got, vec![ev(Some("a"), "1"), ev(None, "2")]);
    }

    #[test]
    fn handles_split_chunks_and_crlf() {
        let mut p = SseParser::default();
        assert!(p.feed(b"data: hel").is_empty());
        assert!(p.feed(b"lo\r\n").is_empty());
        assert_eq!(p.feed(b"\r\n"), vec![ev(None, "hello")]);
    }

    #[test]
    fn joins_multiline_data_and_skips_comments() {
        let mut p = SseParser::default();
        let got = p.feed(b": ping\ndata: a\ndata: b\n\n");
        assert_eq!(got, vec![ev(None, "a\nb")]);
    }

    #[test]
    fn multibyte_split_across_chunks() {
        let mut p = SseParser::default();
        let s = "data: é\n\n".as_bytes();
        assert!(p.feed(&s[..7]).is_empty());
        assert_eq!(p.feed(&s[7..]), vec![ev(None, "é")]);
    }
}

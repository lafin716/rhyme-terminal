use super::agent_status::AgentTaskStatus;

/// Reads only OSC 0/2 title prefixes, never ordinary screen text. Keep parsing
/// across PTY reads, including UTF-8 glyphs and the two-byte ST terminator.
#[derive(Default)]
pub(super) struct ClaudeTitle {
    state: u8,
    payload: Vec<u8>,
    pub status: Option<AgentTaskStatus>,
}

impl ClaudeTitle {
    pub fn feed(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            match self.state {
                0 if byte == 0x1b => self.state = 1,
                1 if byte == b']' => {
                    self.payload.clear();
                    self.state = 2;
                }
                1 => self.state = if byte == 0x1b { 1 } else { 0 },
                2 if byte == 0x07 => self.finish(),
                2 if byte == 0x1b => self.state = 3,
                2 => {
                    // Discard oversized OSCs until their terminator. Do not let
                    // unrelated clipboard/link payloads grow the tracker.
                    if self.payload.len() < 1024 {
                        self.payload.push(byte);
                    } else {
                        self.payload.clear();
                        self.state = 4;
                    }
                }
                3 if byte == b'\\' => self.finish(),
                3 => {
                    self.payload.clear();
                    self.state = if byte == b']' {
                        2
                    } else if byte == 0x1b {
                        1
                    } else {
                        0
                    };
                }
                4 if byte == 0x07 => self.state = 0,
                4 if byte == 0x1b => self.state = 5,
                5 => {
                    self.state = if byte == b'\\' {
                        0
                    } else if byte == 0x1b {
                        5
                    } else {
                        4
                    }
                }
                _ => {}
            }
        }
    }

    fn finish(&mut self) {
        self.state = 0;
        if let Ok(payload) = std::str::from_utf8(&self.payload) {
            if let Some(title) = payload
                .strip_prefix("0;")
                .or_else(|| payload.strip_prefix("2;"))
            {
                let mut chars = title.chars();
                let marker = chars.next();
                // Claude's title is "<activity glyph> <session title>".
                if chars.next() == Some(' ') {
                    match marker {
                        Some('◐' | '◑' | '\u{2801}'..='\u{28ff}') => {
                            self.status = Some(AgentTaskStatus::Working)
                        }
                        Some('✳' | '✱') => self.status = Some(AgentTaskStatus::Completed),
                        _ => {}
                    }
                }
            }
        }
        self.payload.clear();
    }
}

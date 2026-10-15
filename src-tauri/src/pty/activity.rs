// Pure, stateless detection of per-Session activity signals from a single raw
// PTY output chunk. Lives in its own module so it is unit-testable in isolation
// (the pre-agreed cargo test seam), with no I/O or daemon state — mirroring the
// pure-core style used in `commands.rs`.

/// A lightweight, per-chunk activity summary produced by the daemon read
/// thread. `output` is set for any non-empty chunk; `bell` is set only for a
/// real terminal bell (0x07) that is not acting as an OSC-string terminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkSignal {
    pub output: bool,
    pub bell: bool,
}

/// Detect activity signals in a single raw PTY output chunk.
///
/// `output` is simply `!chunk.is_empty()`.
///
/// `bell` counts a BEL (0x07) only when it is *not* terminating an OSC string:
/// shells (e.g. the PowerShell prompt integration this project injects) emit
/// `ESC ] 9 ; 9 ; <path> BEL` on every prompt, and that BEL must not be read as
/// an alert. A tiny state machine tracks OSC context: `ESC ]` opens an OSC
/// string, inside which a BEL — or an `ESC \` (ST) — is the terminator (not a
/// bell); any BEL seen outside an OSC string is a real bell.
///
/// OSC strings that span a chunk boundary are handled best-effort per chunk:
/// no state carries across chunks, so a terminator split across a boundary
/// degrades to treating that BEL as a real bell. This keeps the detector
/// allocation-free and stateless, which matters on the hot read path.
pub fn detect_activity(chunk: &[u8]) -> ChunkSignal {
    const ESC: u8 = 0x1B;
    const BEL: u8 = 0x07;

    let mut bell = false;
    let mut in_osc = false;
    let mut prev_esc = false;

    for &b in chunk {
        if in_osc {
            if b == BEL {
                // OSC terminated by BEL — a redraw/title marker, not an alert.
                in_osc = false;
                prev_esc = false;
            } else if prev_esc && b == b'\\' {
                // OSC terminated by ST (ESC \).
                in_osc = false;
                prev_esc = false;
            } else {
                prev_esc = b == ESC;
            }
            continue;
        }

        if prev_esc && b == b']' {
            // ESC ] opens an OSC string.
            in_osc = true;
            prev_esc = false;
            continue;
        }

        if b == BEL {
            bell = true;
            prev_esc = false;
            continue;
        }

        prev_esc = b == ESC;
    }

    ChunkSignal {
        output: !chunk.is_empty(),
        bell,
    }
}

#[cfg(test)]
mod tests {
    use super::{detect_activity, ChunkSignal};

    #[test]
    fn plain_bell_is_a_bell() {
        assert_eq!(
            detect_activity(b"done\x07"),
            ChunkSignal {
                output: true,
                bell: true
            }
        );
    }

    #[test]
    fn osc_terminator_bel_is_not_a_bell() {
        // The PowerShell prompt integration emits `ESC ] 9 ; 9 ; <path> BEL`.
        assert_eq!(
            detect_activity(b"\x1b]9;9;C:\\x\x07"),
            ChunkSignal {
                output: true,
                bell: false
            }
        );
    }

    #[test]
    fn ordinary_output_is_output_without_bell() {
        assert_eq!(
            detect_activity(b"hello world"),
            ChunkSignal {
                output: true,
                bell: false
            }
        );
    }

    #[test]
    fn empty_chunk_is_no_output() {
        assert_eq!(
            detect_activity(b""),
            ChunkSignal {
                output: false,
                bell: false
            }
        );
    }

    #[test]
    fn st_terminated_osc_is_not_a_bell() {
        // OSC closed by ST (ESC \) rather than BEL — still not an alert.
        assert_eq!(
            detect_activity(b"\x1b]0;title\x1b\\"),
            ChunkSignal {
                output: true,
                bell: false
            }
        );
    }

    #[test]
    fn real_bell_after_an_osc_terminator_still_counts() {
        // First BEL closes the OSC; the second, outside any OSC, is a real bell.
        assert_eq!(
            detect_activity(b"\x1b]9;9;p\x07\x07"),
            ChunkSignal {
                output: true,
                bell: true
            }
        );
    }
}

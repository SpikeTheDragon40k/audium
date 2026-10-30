//! Listening sessions: continuous stretches of playback of one book, the
//! basis of the stats page.

/// Stretches shorter than this (in wall-clock time) are not recorded.
pub const MIN_SESSION_MS: i64 = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Session {
    pub started_at: i64,
    pub ended_at: i64,
    pub start_position_ms: i64,
    pub end_position_ms: i64,
    /// Wall-clock time spent playing.
    pub listened_ms: i64,
}

#[derive(Debug, Clone, Copy)]
struct Open {
    started_at: i64,
    start_position_ms: i64,
    last_position_ms: i64,
    last_at: i64,
}

#[derive(Debug, Clone, Default)]
pub struct SessionTracker {
    open: Option<Open>,
}

impl SessionTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Playback started (or resumed) at `position_ms`.
    pub fn start(&mut self, now_ms: i64, position_ms: i64) {
        if self.open.is_none() {
            self.open = Some(Open {
                started_at: now_ms,
                start_position_ms: position_ms,
                last_position_ms: position_ms,
                last_at: now_ms,
            });
        }
    }

    /// A position update while playing.
    pub fn update(&mut self, now_ms: i64, position_ms: i64) {
        if let Some(o) = &mut self.open {
            o.last_position_ms = position_ms;
            o.last_at = now_ms;
        }
    }

    /// Playback paused, stopped or the book changed. Returns the session to
    /// record, if it was long enough.
    pub fn finish(&mut self, now_ms: i64) -> Option<Session> {
        let o = self.open.take()?;
        let ended_at = now_ms.max(o.last_at);
        let listened_ms = ended_at - o.started_at;
        (listened_ms >= MIN_SESSION_MS).then_some(Session {
            started_at: o.started_at,
            ended_at,
            start_position_ms: o.start_position_ms,
            end_position_ms: o.last_position_ms,
            listened_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_a_stretch_of_playback() {
        let mut t = SessionTracker::new();
        t.start(1_000, 50_000);
        t.start(2_000, 99_999); // already open: ignored
        t.update(31_000, 80_000);
        let s = t.finish(31_500).expect("long enough");
        assert_eq!(
            s,
            Session {
                started_at: 1_000,
                ended_at: 31_500,
                start_position_ms: 50_000,
                end_position_ms: 80_000,
                listened_ms: 30_500
            }
        );
        assert!(!t.is_open());
        assert_eq!(t.finish(40_000), None);
    }

    #[test]
    fn short_stretches_are_dropped() {
        let mut t = SessionTracker::new();
        t.start(0, 0);
        t.update(4_000, 4_000);
        assert_eq!(t.finish(4_500), None);
    }
}

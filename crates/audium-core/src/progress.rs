//! When to save playback progress: debounced while playing, immediately on
//! pause, track change, seeks and shutdown.

/// Minimum time between periodic saves while playing.
pub const SAVE_INTERVAL_MS: i64 = 5_000;
/// A position change this much larger than the elapsed time is a seek.
pub const SEEK_THRESHOLD_MS: i64 = 3_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub track_index: usize,
    pub offset_ms: i64,
    pub book_ms: i64,
}

#[derive(Debug, Clone, Default)]
pub struct ProgressTracker {
    last_saved: Option<Position>,
    last_save_at: i64,
    last_seen: Option<(Position, i64)>,
}

impl ProgressTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// A position update while playing (or after a seek while paused).
    /// Returns true if it should be saved now.
    pub fn on_position(&mut self, pos: Position, now_ms: i64, playing: bool, rate: f64) -> bool {
        let seek = self.last_seen.is_some_and(|(prev, at)| {
            let expected = if playing {
                ((now_ms - at).max(0) as f64 * rate) as i64
            } else {
                0
            };
            (pos.book_ms - prev.book_ms - expected).abs() > SEEK_THRESHOLD_MS
        });
        let track_changed = self
            .last_seen
            .is_some_and(|(prev, _)| prev.track_index != pos.track_index);
        self.last_seen = Some((pos, now_ms));

        let due = playing && now_ms - self.last_save_at >= SAVE_INTERVAL_MS;
        let changed = self.last_saved != Some(pos);
        changed && (seek || track_changed || due)
    }

    /// Pause, stop, end of book or shutdown: save if anything changed.
    pub fn on_stop(&mut self) -> Option<Position> {
        let (pos, _) = self.last_seen?;
        (self.last_saved != Some(pos)).then_some(pos)
    }

    /// The latest position seen.
    pub fn current(&self) -> Option<Position> {
        self.last_seen.map(|(p, _)| p)
    }

    pub fn mark_saved(&mut self, pos: Position, now_ms: i64) {
        self.last_saved = Some(pos);
        self.last_save_at = now_ms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(track_index: usize, offset_ms: i64, book_ms: i64) -> Position {
        Position {
            track_index,
            offset_ms,
            book_ms,
        }
    }

    /// Feeds an update and records the save like the session does.
    fn feed(t: &mut ProgressTracker, pos: Position, now: i64, playing: bool) -> bool {
        let save = t.on_position(pos, now, playing, 1.0);
        if save {
            t.mark_saved(pos, now);
        }
        save
    }

    #[test]
    fn periodic_saves_are_debounced() {
        let mut t = ProgressTracker::new();
        t.mark_saved(p(0, 0, 0), 0);
        let saves: Vec<i64> = (1..=60)
            .map(|i| i * 250)
            .filter(|&now| feed(&mut t, p(0, now, now), now, true))
            .collect();
        assert_eq!(saves, [5_000, 10_000, 15_000]);
    }

    #[test]
    fn track_change_and_seeks_save_immediately() {
        let mut t = ProgressTracker::new();
        t.mark_saved(p(0, 0, 0), 0);
        assert!(!feed(&mut t, p(0, 900, 900), 1_000, true));
        assert!(feed(&mut t, p(1, 0, 1_100), 1_200, true), "track change");
        assert!(
            feed(&mut t, p(1, 60_000, 61_100), 1_400, true),
            "seek forward"
        );
        assert!(feed(&mut t, p(1, 50_000, 51_100), 1_600, true), "seek back");
        // Normal progress right after does not save again.
        assert!(!feed(&mut t, p(1, 50_200, 51_300), 1_800, true));
    }

    #[test]
    fn faster_playback_is_not_mistaken_for_a_seek() {
        let mut t = ProgressTracker::new();
        t.mark_saved(p(0, 0, 0), 0);
        assert!(!t.on_position(p(0, 0, 0), 0, true, 3.0));
        assert!(!t.on_position(p(0, 3_000, 3_000), 1_000, true, 3.0));
    }

    #[test]
    fn seeks_while_paused_save_and_stop_saves_once() {
        let mut t = ProgressTracker::new();
        t.mark_saved(p(0, 1_000, 1_000), 0);
        assert!(!feed(&mut t, p(0, 1_000, 1_000), 10, false));
        assert!(feed(&mut t, p(0, 30_000, 30_000), 20, false));
        assert_eq!(t.on_stop(), None, "already saved");
        assert!(!feed(&mut t, p(0, 30_400, 30_400), 400, true));
        assert_eq!(t.on_stop(), Some(p(0, 30_400, 30_400)));
    }

    #[test]
    fn nothing_seen_nothing_to_save() {
        assert_eq!(ProgressTracker::new().on_stop(), None);
    }
}

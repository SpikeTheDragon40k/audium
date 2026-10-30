//! Sleep timer: stop after some minutes of playback, or at the end of the
//! current chapter, fading the volume out over the last seconds. User
//! activity during the fade extends the timer.
//!
//! Time only counts while playing: a paused timer does not run down.

/// Length of the fade-out at the end.
pub const DEFAULT_FADE_MS: i64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    /// Stop after this much playing time; `left` counts down.
    Minutes { duration_ms: i64, left_ms: i64 },
    /// Stop when the book position reaches `end_ms`.
    EndOfChapter { end_ms: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tick {
    /// Multiply the user's volume by this (1.0 = no fade).
    pub volume_factor: f64,
    /// The timer ran out: pause now and restore the volume.
    pub expired: bool,
}

const NO_FADE: Tick = Tick {
    volume_factor: 1.0,
    expired: false,
};

#[derive(Debug, Clone)]
pub struct SleepTimer {
    mode: Option<Mode>,
    fade_ms: i64,
    /// Wall-clock time left, from the last tick.
    left_ms: i64,
}

impl Default for SleepTimer {
    fn default() -> Self {
        Self::new(DEFAULT_FADE_MS)
    }
}

impl SleepTimer {
    pub fn new(fade_ms: i64) -> Self {
        SleepTimer {
            mode: None,
            fade_ms: fade_ms.max(1),
            left_ms: 0,
        }
    }

    pub fn start_minutes(&mut self, duration_ms: i64) {
        let duration_ms = duration_ms.max(1);
        self.mode = Some(Mode::Minutes {
            duration_ms,
            left_ms: duration_ms,
        });
        self.left_ms = duration_ms;
    }

    /// `end_ms` is the book-wide end of the current chapter.
    pub fn start_end_of_chapter(&mut self, end_ms: i64, position_ms: i64, rate: f64) {
        self.mode = Some(Mode::EndOfChapter { end_ms });
        self.left_ms = wall_ms(end_ms - position_ms, rate);
    }

    pub fn cancel(&mut self) {
        self.mode = None;
        self.left_ms = 0;
    }

    pub fn is_active(&self) -> bool {
        self.mode.is_some()
    }

    pub fn is_end_of_chapter(&self) -> bool {
        matches!(self.mode, Some(Mode::EndOfChapter { .. }))
    }

    pub fn is_fading(&self) -> bool {
        self.is_active() && self.left_ms <= self.fade_ms
    }

    /// Wall-clock playing time until the timer stops playback.
    pub fn remaining_ms(&self) -> i64 {
        if self.is_active() {
            self.left_ms.max(0)
        } else {
            0
        }
    }

    /// Advances the timer by `elapsed_ms` of playing time; `position_ms` and
    /// `rate` are the current book position and speed.
    pub fn tick(&mut self, elapsed_ms: i64, position_ms: i64, rate: f64) -> Tick {
        let Some(mode) = &mut self.mode else {
            return NO_FADE;
        };
        self.left_ms = match mode {
            Mode::Minutes { left_ms, .. } => {
                *left_ms -= elapsed_ms.max(0);
                *left_ms
            }
            Mode::EndOfChapter { end_ms } => wall_ms(*end_ms - position_ms, rate),
        };
        if self.left_ms <= 0 {
            self.cancel();
            return Tick {
                volume_factor: 1.0,
                expired: true,
            };
        }
        let factor = if self.left_ms < self.fade_ms {
            self.left_ms as f64 / self.fade_ms as f64
        } else {
            1.0
        };
        Tick {
            volume_factor: factor,
            expired: false,
        }
    }

    /// The user moved the mouse, pressed a key, used a media key...
    /// During the fade this extends the timer: minutes restart from the full
    /// duration; "end of chapter" moves to `next_chapter_end_ms` (the end of
    /// the following chapter), or keeps playing to the book's end. Returns
    /// true if the timer was extended.
    pub fn activity(
        &mut self,
        next_chapter_end_ms: Option<i64>,
        position_ms: i64,
        rate: f64,
    ) -> bool {
        if !self.is_fading() {
            return false;
        }
        match self.mode {
            Some(Mode::Minutes { duration_ms, .. }) => self.start_minutes(duration_ms),
            Some(Mode::EndOfChapter { .. }) => match next_chapter_end_ms {
                Some(end) => self.start_end_of_chapter(end, position_ms, rate),
                None => self.cancel(),
            },
            None => return false,
        }
        true
    }
}

/// Content time to wall-clock time at `rate`.
fn wall_ms(content_ms: i64, rate: f64) -> i64 {
    let rate = if rate.is_finite() && rate > 0.0 {
        rate
    } else {
        1.0
    };
    (content_ms as f64 / rate).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_count_playing_time_and_fade_out() {
        let mut t = SleepTimer::new(10_000);
        t.start_minutes(60_000);
        assert!(t.is_active() && !t.is_fading());
        assert_eq!(t.tick(45_000, 0, 1.0), NO_FADE);
        assert_eq!(t.remaining_ms(), 15_000);
        // Pausing is simply not ticking: nothing runs down.
        let fading = t.tick(10_000, 0, 1.0);
        assert!(t.is_fading());
        assert!((fading.volume_factor - 0.5).abs() < 1e-9);
        let end = t.tick(5_000, 0, 1.0);
        assert!(end.expired && !t.is_active());
        assert_eq!(t.tick(1_000, 0, 1.0), NO_FADE, "inactive afterwards");
    }

    #[test]
    fn end_of_chapter_follows_position_and_speed() {
        let mut t = SleepTimer::new(10_000);
        t.start_end_of_chapter(100_000, 40_000, 2.0);
        assert_eq!(t.remaining_ms(), 30_000, "60 s of content at 2x");
        assert_eq!(t.tick(0, 70_000, 2.0).volume_factor, 1.0);
        let f = t.tick(0, 90_000, 2.0);
        assert!((f.volume_factor - 0.5).abs() < 1e-9, "5 s wall time left");
        assert!(t.tick(0, 100_000, 2.0).expired);
    }

    #[test]
    fn a_seek_past_the_chapter_end_expires() {
        let mut t = SleepTimer::default();
        t.start_end_of_chapter(10_000, 0, 1.0);
        assert!(t.tick(0, 25_000, 1.0).expired);
    }

    #[test]
    fn activity_extends_only_during_the_fade() {
        let mut t = SleepTimer::new(10_000);
        t.start_minutes(60_000);
        assert!(!t.activity(None, 0, 1.0), "not fading yet");
        t.tick(55_000, 0, 1.0);
        assert!(t.activity(None, 0, 1.0));
        assert_eq!(t.remaining_ms(), 60_000);
        assert_eq!(t.tick(1_000, 0, 1.0).volume_factor, 1.0);

        let mut c = SleepTimer::new(10_000);
        c.start_end_of_chapter(100_000, 0, 1.0);
        c.tick(0, 95_000, 1.0);
        assert!(c.activity(Some(160_000), 95_000, 1.0));
        assert!(c.is_end_of_chapter());
        assert_eq!(c.remaining_ms(), 65_000);

        // In the last chapter there is no next end: keep playing.
        c.tick(0, 155_000, 1.0);
        assert!(c.activity(None, 155_000, 1.0));
        assert!(!c.is_active());
    }

    #[test]
    fn cancel_and_bad_rates() {
        let mut t = SleepTimer::default();
        t.start_end_of_chapter(10_000, 0, f64::NAN);
        assert_eq!(t.remaining_ms(), 10_000);
        t.cancel();
        assert!(!t.is_active() && !t.is_fading());
        assert_eq!(t.remaining_ms(), 0);
    }
}

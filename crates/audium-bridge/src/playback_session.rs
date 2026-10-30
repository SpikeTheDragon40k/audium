//! `PlaybackSession`: the playback policy for library books.
//!
//! It decides what to load (tracks, chapters, resume point, the book's
//! speed), saves progress and listening sessions, marks books finished, and
//! runs the sleep timer. It never plays audio itself: `app/src/sessionwiring.cpp`
//! connects it to the C++ `PlaybackController` on the Qt thread. Writes go
//! through a [`Writer`] thread so they never block the UI.

use std::path::Path;
use std::pin::Pin;

use audium_core::listening::SessionTracker;
use audium_core::position::resume_point;
use audium_core::progress::{Position, ProgressTracker};
use audium_core::sleep::{DEFAULT_FADE_MS, SleepTimer};
use audium_core::{BookId, TrackId};
use audium_storage::organize;
use audium_storage::playback::{self, SessionRecord};
use audium_storage::{ReadOnly, Writer, now_ms, read};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl};

use crate::convert::{file_url, playback_path_bytes, qs};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    /// A track to load: libmpv path bytes (native on Unix, UTF-8 on Windows).
    struct SessionTrack {
        path: Vec<u8>,
        title: QString,
        duration_ms: i64,
    }

    /// A chapter with a book-wide start.
    struct SessionChapter {
        title: QString,
        start_ms: i64,
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_uncreatable]
        /// The library database; set by the application once it is open.
        #[qproperty(QString, database_path, cxx_name = "databasePath")]
        #[qproperty(i32, skip_back_ms, cxx_name = "skipBackMs")]
        #[qproperty(i32, skip_forward_ms, cxx_name = "skipForwardMs")]
        /// Length of the sleep timer's fade-out.
        #[qproperty(i32, fade_ms, cxx_name = "fadeMs")]
        /// The library book being played (0: none, e.g. "Open Files").
        #[qproperty(i64, book_id, cxx_name = "bookId", READ, NOTIFY)]
        #[qproperty(QString, book_title, cxx_name = "bookTitle", READ, NOTIFY)]
        #[qproperty(QUrl, cover_url, cxx_name = "coverUrl", READ, NOTIFY)]
        #[qproperty(bool, sleep_active, cxx_name = "sleepActive", READ, NOTIFY)]
        #[qproperty(
            bool,
            sleep_end_of_chapter,
            cxx_name = "sleepEndOfChapter",
            READ,
            NOTIFY
        )]
        #[qproperty(bool, sleep_fading, cxx_name = "sleepFading", READ, NOTIFY)]
        /// Playing time left on the sleep timer, rounded up to seconds.
        #[qproperty(i64, sleep_remaining_ms, cxx_name = "sleepRemainingMs", READ, NOTIFY)]
        /// Increases after saved progress changed in a way the library
        /// shows (pause, stop, finished, another book); models reload on it.
        #[qproperty(i32, revision, READ, NOTIFY)]
        /// The current book's authors, comma-separated.
        #[qproperty(QString, book_authors, cxx_name = "bookAuthors", READ, NOTIFY)]
        /// Closing the window while a book plays hides it to the tray.
        #[qproperty(bool, close_to_tray, cxx_name = "closeToTray")]
        /// Colour theme: 0 follows the desktop, 1 Dracula, 2 Nord (applied
        /// by the C++ Theme object).
        #[qproperty(i32, theme)]
        type PlaybackSession = super::PlaybackSessionRust;

        /// Loads a library book (resuming where it was left) and plays it.
        /// Returns false if the book has no playable file.
        #[qinvokable]
        #[cxx_name = "playBook"]
        fn play_book(self: Pin<&mut PlaybackSession>, book_id: i64) -> bool;

        /// Plays a library book from `position_ms` (book-wide), e.g. a chapter
        /// or a bookmark; seeks if the book is already playing.
        #[qinvokable]
        #[cxx_name = "playBookAt"]
        fn play_book_at(self: Pin<&mut PlaybackSession>, book_id: i64, position_ms: i64) -> bool;

        /// Bookmarks the current position of the current book. Returns false
        /// when no library book is playing.
        #[qinvokable]
        #[cxx_name = "addBookmark"]
        fn add_bookmark(self: Pin<&mut PlaybackSession>) -> bool;

        #[qinvokable]
        #[cxx_name = "updateBookmark"]
        fn update_bookmark(
            self: Pin<&mut PlaybackSession>,
            id: i64,
            title: &QString,
            note: &QString,
        );

        #[qinvokable]
        #[cxx_name = "deleteBookmark"]
        fn delete_bookmark(self: Pin<&mut PlaybackSession>, id: i64);

        /// Play the current book from a book-wide position (seek, then play).
        #[qsignal]
        #[cxx_name = "seekRequested"]
        fn seek_requested(self: Pin<&mut PlaybackSession>, position_ms: i64);

        #[qinvokable]
        #[cxx_name = "startSleepTimer"]
        fn start_sleep_timer(self: Pin<&mut PlaybackSession>, duration_ms: i64);

        #[qinvokable]
        #[cxx_name = "sleepAtEndOfChapter"]
        fn sleep_at_end_of_chapter(self: Pin<&mut PlaybackSession>);

        #[qinvokable]
        #[cxx_name = "cancelSleepTimer"]
        fn cancel_sleep_timer(self: Pin<&mut PlaybackSession>);

        /// Mouse, keyboard or media-key activity: extends a fading timer.
        #[qinvokable]
        #[cxx_name = "userActivity"]
        fn user_activity(self: Pin<&mut PlaybackSession>);

        /// A book is ready to load: read `pending*()` and load it.
        #[qsignal]
        #[cxx_name = "loadRequested"]
        fn load_requested(self: Pin<&mut PlaybackSession>);

        #[qsignal]
        #[cxx_name = "pauseRequested"]
        fn pause_requested(self: Pin<&mut PlaybackSession>);

        /// A library book was played to its end.
        #[qsignal]
        #[cxx_name = "bookFinished"]
        fn book_finished(self: Pin<&mut PlaybackSession>, title: QString);

        /// The sleep timer paused playback.
        #[qsignal]
        #[cxx_name = "sleepTimerExpired"]
        fn sleep_timer_expired(self: Pin<&mut PlaybackSession>);

        /// Output volume multiplier for the sleep fade (1.0 = none).
        #[qsignal]
        #[cxx_name = "fadeRequested"]
        fn fade_requested(self: Pin<&mut PlaybackSession>, factor: f64);
    }

    impl cxx_qt::Initialize for PlaybackSession {}
    impl cxx_qt::Threading for PlaybackSession {}

    // Called by the C++ wiring.
    unsafe extern "RustQt" {
        #[cxx_name = "pendingTracks"]
        fn pending_tracks(self: &PlaybackSession) -> Vec<SessionTrack>;
        #[cxx_name = "pendingChapters"]
        fn pending_chapters(self: &PlaybackSession) -> Vec<SessionChapter>;
        #[cxx_name = "pendingStartTrack"]
        fn pending_start_track(self: &PlaybackSession) -> i32;
        #[cxx_name = "pendingStartOffset"]
        fn pending_start_offset(self: &PlaybackSession) -> i64;
        #[cxx_name = "pendingRate"]
        fn pending_rate(self: &PlaybackSession) -> f64;

        /// The controller's state, as `PlaybackController::State` (0 stopped,
        /// 1 loading, 2 paused, 3 playing, 4 error).
        #[cxx_name = "onState"]
        fn on_state(self: Pin<&mut PlaybackSession>, state: i32);
        #[cxx_name = "onPosition"]
        fn on_position(
            self: Pin<&mut PlaybackSession>,
            track_index: i32,
            offset_ms: i64,
            book_ms: i64,
        );
        #[cxx_name = "onRate"]
        fn on_rate(self: Pin<&mut PlaybackSession>, rate: f64);
        #[cxx_name = "onEndOfBook"]
        fn on_end_of_book(self: Pin<&mut PlaybackSession>);
        /// Something else was loaded into the player: stop tracking the book.
        fn detach(self: Pin<&mut PlaybackSession>);
        /// Saves everything and waits for the writes (application quit).
        fn shutdown(self: Pin<&mut PlaybackSession>);
    }
}

const STATE_PAUSED: i32 = 2;
const STATE_PLAYING: i32 = 3;
const DEFAULT_SKIP_BACK_MS: i32 = 15_000;
const DEFAULT_SKIP_FORWARD_MS: i32 = 30_000;
const KEY_SKIP_BACK: &str = "skip_back_ms";
const KEY_SKIP_FORWARD: &str = "skip_forward_ms";
const KEY_CLOSE_TO_TRAY: &str = "close_to_tray";
const KEY_THEME: &str = "theme";
/// Themes are 0 (system), 1 (Dracula) and 2 (Nord).
const THEMES: std::ops::RangeInclusive<i32> = 0..=2;

struct LoadedTrack {
    id: TrackId,
    path: Vec<u8>,
    title: String,
    duration_ms: i64,
}

/// The book currently in the player.
struct Current {
    book: BookId,
    tracks: Vec<LoadedTrack>,
    /// Book-wide chapter starts (from the database, or one per track).
    chapters: Vec<(String, i64)>,
    start: (usize, i64),
    rate: f64,
    /// The player reached Paused/Playing after loading this book.
    ready: bool,
    finished: bool,
}

pub struct PlaybackSessionRust {
    database_path: QString,
    skip_back_ms: i32,
    skip_forward_ms: i32,
    fade_ms: i32,
    book_id: i64,
    book_title: QString,
    cover_url: QUrl,
    sleep_active: bool,
    sleep_end_of_chapter: bool,
    sleep_fading: bool,
    sleep_remaining_ms: i64,
    revision: i32,
    book_authors: QString,
    close_to_tray: bool,
    theme: i32,

    writer: Option<Writer>,
    current: Option<Current>,
    progress: ProgressTracker,
    listening: SessionTracker,
    sleep: SleepTimer,
    playing: bool,
    rate: f64,
    position: Option<Position>,
    last_position_at: i64,
    fade_factor: f64,
}

impl Default for PlaybackSessionRust {
    fn default() -> Self {
        PlaybackSessionRust {
            database_path: QString::default(),
            skip_back_ms: DEFAULT_SKIP_BACK_MS,
            skip_forward_ms: DEFAULT_SKIP_FORWARD_MS,
            fade_ms: DEFAULT_FADE_MS as i32,
            book_id: 0,
            book_title: QString::default(),
            cover_url: QUrl::default(),
            sleep_active: false,
            sleep_end_of_chapter: false,
            sleep_fading: false,
            sleep_remaining_ms: 0,
            revision: 0,
            book_authors: QString::default(),
            close_to_tray: true,
            theme: 0,
            writer: None,
            current: None,
            progress: ProgressTracker::new(),
            listening: SessionTracker::new(),
            sleep: SleepTimer::default(),
            playing: false,
            rate: 1.0,
            position: None,
            last_position_at: 0,
            fade_factor: 1.0,
        }
    }
}

impl cxx_qt::Initialize for qobject::PlaybackSession {
    fn initialize(self: Pin<&mut Self>) {
        let mut this = self;
        this.as_mut()
            .on_database_path_changed(|s| s.open_database())
            .release();
        this.as_mut()
            .on_skip_back_ms_changed(|s| s.save_setting(KEY_SKIP_BACK, s.skip_back_ms))
            .release();
        this.as_mut()
            .on_skip_forward_ms_changed(|s| s.save_setting(KEY_SKIP_FORWARD, s.skip_forward_ms))
            .release();
        this.as_mut()
            .on_close_to_tray_changed(|s| {
                s.save_setting(KEY_CLOSE_TO_TRAY, i32::from(s.close_to_tray))
            })
            .release();
        this.as_mut()
            .on_theme_changed(|s| s.save_setting(KEY_THEME, s.theme))
            .release();
    }
}

type LoadedBook = (
    Vec<LoadedTrack>,
    Vec<(String, i64)>,
    Option<playback::Progress>,
    Option<f64>,
    read::BookSummary,
);

fn load_book(
    db_path: &str,
    book: BookId,
) -> Result<Option<LoadedBook>, audium_storage::StorageError> {
    let db = ReadOnly::open(Path::new(db_path))?;
    let Some(summary) = read::book_summary(db.conn(), book)? else {
        return Ok(None);
    };
    let rows = read::tracks(db.conn(), book)?;
    let any_missing = rows.iter().any(|t| t.missing);
    let tracks: Vec<LoadedTrack> = rows
        .into_iter()
        .filter(|t| !t.missing)
        .map(|t| LoadedTrack {
            id: t.id,
            path: playback_path_bytes(&t.path),
            title: t.title.unwrap_or_else(|| t.display_path.clone()),
            duration_ms: t.duration_ms,
        })
        .collect();
    // Stored chapters assume every track is present; otherwise one per track.
    let chapters = if any_missing {
        let mut start = 0;
        tracks
            .iter()
            .map(|t| {
                let c = (t.title.clone(), start);
                start += t.duration_ms.max(0);
                c
            })
            .collect()
    } else {
        read::chapters(db.conn(), book)?
            .into_iter()
            .map(|c| (c.title, c.start_ms))
            .collect()
    };
    let progress = playback::progress(db.conn(), book)?;
    let speed = playback::book_speed(db.conn(), book)?;
    Ok(Some((tracks, chapters, progress, speed, summary)))
}

impl qobject::PlaybackSession {
    fn open_database(mut self: Pin<&mut Self>) {
        let path = String::from(&self.database_path);
        let writer = if path.is_empty() {
            None
        } else {
            Writer::open(Path::new(&path))
                .inspect_err(
                    |e| tracing::error!(error = %e, "cannot open the database for playback"),
                )
                .ok()
        };
        // Dropping the old writer finishes its queued writes.
        self.as_mut().rust_mut().writer = writer;
        let Ok(db) = ReadOnly::open(Path::new(&path)) else {
            return;
        };
        let read_ms = |key: &str, default: i32| {
            playback::setting(db.conn(), key)
                .ok()
                .flatten()
                .and_then(|v| v.parse::<i32>().ok())
                .filter(|v| (1_000..=600_000).contains(v))
                .unwrap_or(default)
        };
        let back = read_ms(KEY_SKIP_BACK, DEFAULT_SKIP_BACK_MS);
        let forward = read_ms(KEY_SKIP_FORWARD, DEFAULT_SKIP_FORWARD_MS);
        let close_to_tray = playback::setting(db.conn(), KEY_CLOSE_TO_TRAY)
            .ok()
            .flatten()
            .is_none_or(|v| v != "0");
        self.as_mut().set_skip_back_ms(back);
        self.as_mut().set_skip_forward_ms(forward);
        let theme = playback::setting(db.conn(), KEY_THEME)
            .ok()
            .flatten()
            .and_then(|v| v.parse::<i32>().ok())
            .filter(|v| THEMES.contains(v))
            .unwrap_or(0);
        self.as_mut().set_close_to_tray(close_to_tray);
        self.as_mut().set_theme(theme);
    }

    fn save_setting(&self, key: &'static str, value: i32) {
        if let Some(w) = &self.writer {
            w.submit(move |db| {
                if let Err(e) = playback::set_setting(db.conn(), key, &value.to_string()) {
                    tracing::error!(error = %e, key, "cannot save setting");
                }
            });
        }
    }

    fn play_book(self: Pin<&mut Self>, book_id: i64) -> bool {
        self.start_book(book_id, None)
    }

    fn play_book_at(mut self: Pin<&mut Self>, book_id: i64, position_ms: i64) -> bool {
        // Already playing this book: just seek. (A finished book is loaded
        // again, so that progress and the sleep timer work after it.)
        if self
            .current
            .as_ref()
            .is_some_and(|c| c.book.0 == book_id && c.ready && !c.finished)
        {
            self.as_mut().seek_requested(position_ms.max(0));
            return true;
        }
        self.start_book(book_id, Some(position_ms.max(0)))
    }

    /// Loads a book at `at` (book-wide), or where it was left.
    fn start_book(mut self: Pin<&mut Self>, book_id: i64, at: Option<i64>) -> bool {
        let path = String::from(&self.database_path);
        let loaded = match load_book(&path, BookId(book_id)) {
            Ok(Some(l)) => l,
            Ok(None) => return false,
            Err(e) => {
                tracing::error!(error = %e, "cannot load book");
                return false;
            }
        };
        let (tracks, chapters, progress, speed, summary) = loaded;
        let was_finished = progress.is_some_and(|p| p.finished);
        if tracks.is_empty() {
            return false;
        }
        self.as_mut().leave_book();

        // A finished book starts again from the beginning.
        let ids: Vec<(TrackId, u64)> = tracks
            .iter()
            .map(|t| (t.id, t.duration_ms.max(0) as u64))
            .collect();
        let durations: Vec<u64> = ids.iter().map(|(_, d)| *d).collect();
        let start = match (at, progress.filter(|p| !p.finished)) {
            (Some(ms), _) => audium_core::position::track_position(&durations, ms as u64)
                .map(|(i, o)| (i, o as i64))
                .unwrap_or((0, 0)),
            (None, Some(p)) => resume_point(
                &ids,
                p.track_id.as_ref(),
                p.offset_ms.max(0) as u64,
                p.book_position_ms.max(0) as u64,
            )
            .map(|(i, o)| (i, o as i64))
            .unwrap_or((0, 0)),
            (None, None) => (0, 0),
        };
        let rate = speed.unwrap_or(1.0).clamp(0.5, 3.0);
        let book_ms = tracks
            .iter()
            .take(start.0)
            .map(|t| t.duration_ms.max(0))
            .sum::<i64>()
            + start.1;
        let initial = Position {
            track_index: start.0,
            offset_ms: start.1,
            book_ms,
        };
        {
            let mut rust = self.as_mut().rust_mut();
            rust.progress = ProgressTracker::new();
            rust.progress.mark_saved(initial, now_ms());
            rust.position = Some(initial);
            rust.current = Some(Current {
                book: BookId(book_id),
                tracks,
                chapters,
                start,
                rate,
                ready: false,
                finished: false,
            });
        }
        // Starting a finished book again restarts it: save the new start now,
        // so it leaves "Finished" even if it is paused right away.
        if was_finished {
            self.as_mut().write_progress(initial, now_ms());
        }
        set_ro!(self, book_id, book_id_changed, book_id);
        set_ro!(self, book_title, book_title_changed, qs(&summary.title));
        set_ro!(
            self,
            book_authors,
            book_authors_changed,
            qs(&summary.authors.join(", "))
        );
        set_ro!(
            self,
            cover_url,
            cover_url_changed,
            file_url(summary.cover_path.as_deref())
        );
        self.as_mut().load_requested();
        true
    }

    fn pending_tracks(&self) -> Vec<qobject::SessionTrack> {
        self.current
            .iter()
            .flat_map(|c| &c.tracks)
            .map(|t| qobject::SessionTrack {
                path: t.path.clone(),
                title: qs(&t.title),
                duration_ms: t.duration_ms,
            })
            .collect()
    }

    fn pending_chapters(&self) -> Vec<qobject::SessionChapter> {
        self.current
            .iter()
            .flat_map(|c| &c.chapters)
            .map(|(title, start)| qobject::SessionChapter {
                title: qs(title),
                start_ms: *start,
            })
            .collect()
    }

    fn pending_start_track(&self) -> i32 {
        self.current
            .as_ref()
            .map_or(0, |c| i32::try_from(c.start.0).unwrap_or(0))
    }

    fn pending_start_offset(&self) -> i64 {
        self.current.as_ref().map_or(0, |c| c.start.1)
    }

    fn pending_rate(&self) -> f64 {
        self.current.as_ref().map_or(1.0, |c| c.rate)
    }

    fn on_state(mut self: Pin<&mut Self>, state: i32) {
        let now = now_ms();
        let playing = state == STATE_PLAYING;
        if let Some(c) = self.as_mut().rust_mut().current.as_mut()
            && (state == STATE_PAUSED || playing)
        {
            c.ready = true;
        }
        if playing == self.playing {
            return;
        }
        self.as_mut().rust_mut().playing = playing;
        self.as_mut().rust_mut().last_position_at = now;
        if playing {
            let pos = self.position.map_or(0, |p| p.book_ms);
            self.as_mut().rust_mut().listening.start(now, pos);
        } else {
            self.as_mut().save_now(now);
            self.as_mut().finish_session(now);
            self.as_mut().notify_after_writes();
        }
    }

    fn on_position(mut self: Pin<&mut Self>, track_index: i32, offset_ms: i64, book_ms: i64) {
        let Some(c) = self.current.as_ref() else {
            return;
        };
        if !c.ready || c.finished || track_index < 0 || track_index as usize >= c.tracks.len() {
            return;
        }
        let now = now_ms();
        let pos = Position {
            track_index: track_index as usize,
            offset_ms,
            book_ms,
        };
        let elapsed = if self.playing {
            (now - self.last_position_at).max(0)
        } else {
            0
        };
        let (playing, rate) = (self.playing, self.rate);
        {
            let mut rust = self.as_mut().rust_mut();
            rust.position = Some(pos);
            rust.last_position_at = now;
            rust.listening.update(now, book_ms);
        }
        if self
            .as_mut()
            .rust_mut()
            .progress
            .on_position(pos, now, playing, rate)
        {
            self.as_mut().write_progress(pos, now);
        }
        if playing && self.sleep.is_active() {
            let tick = self.as_mut().rust_mut().sleep.tick(elapsed, book_ms, rate);
            if tick.expired {
                self.as_mut().apply_fade(1.0);
                self.as_mut().pause_requested();
                self.as_mut().sleep_timer_expired();
            } else {
                self.as_mut().apply_fade(tick.volume_factor);
            }
            self.as_mut().publish_sleep();
        }
    }

    fn on_rate(mut self: Pin<&mut Self>, rate: f64) {
        let book = {
            let mut rust = self.as_mut().rust_mut();
            rust.rate = rate;
            let Some(c) = rust.current.as_mut() else {
                return;
            };
            if !c.ready || (c.rate - rate).abs() < 1e-6 {
                return;
            }
            c.rate = rate;
            c.book
        };
        if let Some(w) = &self.writer {
            w.submit(move |db| {
                if let Err(e) = playback::set_book_speed(db.conn(), book, rate) {
                    tracing::error!(error = %e, "cannot save the book's speed");
                }
            });
        }
    }

    fn on_end_of_book(mut self: Pin<&mut Self>) {
        let now = now_ms();
        let Some(c) = self.current.as_ref() else {
            return;
        };
        if !c.ready || c.finished {
            return;
        }
        let last = c.tracks.len() - 1;
        let book = c.book;
        let track = c.tracks[last].id;
        let offset = c.tracks[last].duration_ms.max(0);
        let total: i64 = c.tracks.iter().map(|t| t.duration_ms.max(0)).sum();
        let end = Position {
            track_index: last,
            offset_ms: offset,
            book_ms: total,
        };
        if let Some(c) = self.as_mut().rust_mut().current.as_mut() {
            c.finished = true;
        }
        // The final position is saved with the "finished" mark; nothing may
        // save it again afterwards, or the book would count as restarted.
        self.as_mut().rust_mut().progress.mark_saved(end, now);
        self.as_mut().rust_mut().listening.update(now, total);
        self.as_mut().finish_session(now);
        if let Some(w) = &self.writer {
            w.submit(move |db| {
                if let Err(e) =
                    playback::mark_finished(db.conn(), book, Some(track), offset, total, now)
                {
                    tracing::error!(error = %e, "cannot mark the book finished");
                }
            });
        }
        self.as_mut().notify_after_writes();
        let title = self.book_title.clone();
        self.as_mut().book_finished(title);
    }

    fn detach(mut self: Pin<&mut Self>) {
        self.as_mut().leave_book();
    }

    fn shutdown(mut self: Pin<&mut Self>) {
        self.as_mut().leave_book();
        if let Some(w) = &self.writer {
            w.flush();
        }
    }

    /// Saves and closes everything about the current book.
    fn leave_book(mut self: Pin<&mut Self>) {
        let now = now_ms();
        self.as_mut().save_now(now);
        self.as_mut().finish_session(now);
        self.as_mut().cancel_sleep_timer();
        self.as_mut().rust_mut().current = None;
        self.as_mut().rust_mut().position = None;
        self.as_mut().rust_mut().playing = false;
        set_ro!(self, book_id, book_id_changed, 0);
        set_ro!(self, book_title, book_title_changed, QString::default());
        set_ro!(self, book_authors, book_authors_changed, QString::default());
        set_ro!(self, cover_url, cover_url_changed, QUrl::default());
        self.as_mut().notify_after_writes();
    }

    /// Bumps `revision` once every write queued so far is committed.
    fn notify_after_writes(self: Pin<&mut Self>) {
        let Some(w) = &self.writer else {
            return;
        };
        let thread = self.qt_thread();
        w.submit(move |_| {
            let _ = thread.queue(|mut s| {
                let next = s.revision.wrapping_add(1);
                set_ro!(s, revision, revision_changed, next);
            });
        });
    }

    /// Saves the latest position if it changed since the last save.
    fn save_now(mut self: Pin<&mut Self>, now: i64) {
        let ready = self
            .current
            .as_ref()
            .is_some_and(|c| c.ready && !c.finished);
        if !ready {
            return;
        }
        if let Some(pos) = self.as_mut().rust_mut().progress.on_stop() {
            self.as_mut().write_progress(pos, now);
        }
    }

    fn write_progress(mut self: Pin<&mut Self>, pos: Position, now: i64) {
        let Some(c) = self.current.as_ref() else {
            return;
        };
        let book = c.book;
        let track = c.tracks.get(pos.track_index).map(|t| t.id);
        self.as_mut().rust_mut().progress.mark_saved(pos, now);
        if let Some(w) = &self.writer {
            w.submit(move |db| {
                if let Err(e) =
                    playback::save_progress(db.conn(), book, track, pos.offset_ms, pos.book_ms, now)
                {
                    tracing::error!(error = %e, "cannot save progress");
                }
            });
        }
    }

    fn finish_session(mut self: Pin<&mut Self>, now: i64) {
        let Some(book) = self.current.as_ref().map(|c| c.book) else {
            self.as_mut().rust_mut().listening = SessionTracker::new();
            return;
        };
        let Some(s) = self.as_mut().rust_mut().listening.finish(now) else {
            return;
        };
        if let Some(w) = &self.writer {
            let record = SessionRecord {
                book,
                started_at: s.started_at,
                ended_at: s.ended_at,
                start_position_ms: s.start_position_ms,
                end_position_ms: s.end_position_ms,
                listened_ms: s.listened_ms,
            };
            w.submit(move |db| {
                if let Err(e) = playback::record_session(db.conn(), &record) {
                    tracing::error!(error = %e, "cannot record listening session");
                }
            });
        }
    }

    fn add_bookmark(mut self: Pin<&mut Self>) -> bool {
        let (Some(c), Some(pos)) = (self.current.as_ref(), self.position) else {
            return false;
        };
        if !c.ready {
            return false;
        }
        let book = c.book;
        let track = c.tracks.get(pos.track_index).map(|t| t.id);
        let now = now_ms();
        if let Some(w) = &self.writer {
            w.submit(move |db| {
                if let Err(e) =
                    organize::add_bookmark(db.conn(), book, track, pos.offset_ms, pos.book_ms, now)
                {
                    tracing::error!(error = %e, "cannot add bookmark");
                }
            });
        }
        self.as_mut().notify_after_writes();
        true
    }

    fn update_bookmark(mut self: Pin<&mut Self>, id: i64, title: &QString, note: &QString) {
        let (title, note) = (String::from(title), String::from(note));
        let now = now_ms();
        if let Some(w) = &self.writer {
            w.submit(move |db| {
                if let Err(e) = organize::update_bookmark(db.conn(), id, &title, &note, now) {
                    tracing::error!(error = %e, "cannot update bookmark");
                }
            });
        }
        self.as_mut().notify_after_writes();
    }

    fn delete_bookmark(mut self: Pin<&mut Self>, id: i64) {
        if let Some(w) = &self.writer {
            w.submit(move |db| {
                if let Err(e) = organize::delete_bookmark(db.conn(), id) {
                    tracing::error!(error = %e, "cannot delete bookmark");
                }
            });
        }
        self.as_mut().notify_after_writes();
    }

    fn start_sleep_timer(mut self: Pin<&mut Self>, duration_ms: i64) {
        let fade = i64::from(self.fade_ms);
        let mut timer = SleepTimer::new(fade);
        timer.start_minutes(duration_ms);
        self.as_mut().rust_mut().sleep = timer;
        self.as_mut().apply_fade(1.0);
        self.publish_sleep();
    }

    fn sleep_at_end_of_chapter(mut self: Pin<&mut Self>) {
        let Some(pos) = self.position else {
            return;
        };
        let Some(end) = self.chapter_end_after(pos.book_ms) else {
            return;
        };
        let fade = i64::from(self.fade_ms);
        let rate = self.rate;
        let mut timer = SleepTimer::new(fade);
        timer.start_end_of_chapter(end, pos.book_ms, rate);
        self.as_mut().rust_mut().sleep = timer;
        self.as_mut().apply_fade(1.0);
        self.publish_sleep();
    }

    fn cancel_sleep_timer(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().sleep.cancel();
        self.as_mut().apply_fade(1.0);
        self.publish_sleep();
    }

    fn user_activity(mut self: Pin<&mut Self>) {
        if !self.sleep.is_fading() {
            return;
        }
        let pos = self.position.map_or(0, |p| p.book_ms);
        // "End of chapter" moves on to the end of the next chapter.
        let current_end = self.chapter_end_after(pos);
        let next_end = current_end.and_then(|e| self.chapter_end_after(e));
        let rate = self.rate;
        if self.as_mut().rust_mut().sleep.activity(next_end, pos, rate) {
            self.as_mut().apply_fade(1.0);
            self.publish_sleep();
        }
    }

    /// The end of the chapter that contains `book_ms` (the next chapter's
    /// start, or the end of the book).
    fn chapter_end_after(&self, book_ms: i64) -> Option<i64> {
        let c = self.current.as_ref()?;
        let total: i64 = c.tracks.iter().map(|t| t.duration_ms.max(0)).sum();
        let next = c
            .chapters
            .iter()
            .map(|(_, s)| *s)
            .filter(|&s| s > book_ms)
            .min();
        match next {
            Some(s) => Some(s),
            None if book_ms < total => Some(total),
            None => None,
        }
    }

    fn apply_fade(mut self: Pin<&mut Self>, factor: f64) {
        if (self.fade_factor - factor).abs() >= 0.01 || (factor == 1.0 && self.fade_factor != 1.0) {
            self.as_mut().rust_mut().fade_factor = factor;
            self.as_mut().fade_requested(factor);
        }
    }

    fn publish_sleep(mut self: Pin<&mut Self>) {
        let active = self.sleep.is_active();
        let eoc = self.sleep.is_end_of_chapter();
        let fading = self.sleep.is_fading();
        // Rounded up to whole seconds so the display does not flicker.
        let remaining = (self.sleep.remaining_ms() + 999) / 1000 * 1000;
        set_ro!(self, sleep_active, sleep_active_changed, active);
        set_ro!(
            self,
            sleep_end_of_chapter,
            sleep_end_of_chapter_changed,
            eoc
        );
        set_ro!(self, sleep_fading, sleep_fading_changed, fading);
        set_ro!(
            self,
            sleep_remaining_ms,
            sleep_remaining_ms_changed,
            remaining
        );
    }
}

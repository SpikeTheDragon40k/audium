//! `StatsModel`: listening statistics for the stats page.

use std::path::Path;
use std::pin::Pin;

use audium_core::stats::{Date, summarize};
use audium_storage::{ReadOnly, stats};
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QList, QString, QVariant};

use crate::convert::{file_url, qs, variant_map};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qlist.h");
        type QList_QVariant = cxx_qt_lib::QList<cxx_qt_lib::QVariant>;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, database_path, cxx_name = "databasePath")]
        #[qproperty(i32, revision)]
        /// Number of days in `daily` (the chart).
        #[qproperty(i32, days)]
        #[qproperty(i64, today_ms, cxx_name = "todayMs", READ, NOTIFY)]
        #[qproperty(i64, week_ms, cxx_name = "weekMs", READ, NOTIFY)]
        #[qproperty(i64, month_ms, cxx_name = "monthMs", READ, NOTIFY)]
        #[qproperty(i64, total_ms, cxx_name = "totalMs", READ, NOTIFY)]
        #[qproperty(i32, streak_days, cxx_name = "streakDays", READ, NOTIFY)]
        #[qproperty(i32, best_streak_days, cxx_name = "bestStreakDays", READ, NOTIFY)]
        #[qproperty(i32, finished_total, cxx_name = "finishedTotal", READ, NOTIFY)]
        #[qproperty(i32, finished_month, cxx_name = "finishedMonth", READ, NOTIFY)]
        /// Oldest first: {date "YYYY-MM-DD", year, month, day, weekday (0 Mon), ms}.
        #[qproperty(QList_QVariant, daily, READ, NOTIFY)]
        #[qproperty(i64, max_daily_ms, cxx_name = "maxDailyMs", READ, NOTIFY)]
        /// Most listened books: {bookId, title, coverUrl, ms, sessions}.
        #[qproperty(QList_QVariant, books, READ, NOTIFY)]
        type StatsModel = super::StatsModelRust;
    }

    impl cxx_qt::Initialize for StatsModel {}

    unsafe extern "RustQt" {
        #[qinvokable]
        fn reload(self: Pin<&mut StatsModel>);
    }
}

pub struct StatsModelRust {
    database_path: QString,
    revision: i32,
    days: i32,
    today_ms: i64,
    week_ms: i64,
    month_ms: i64,
    total_ms: i64,
    streak_days: i32,
    best_streak_days: i32,
    finished_total: i32,
    finished_month: i32,
    daily: QList<QVariant>,
    max_daily_ms: i64,
    books: QList<QVariant>,
}

impl Default for StatsModelRust {
    fn default() -> Self {
        StatsModelRust {
            database_path: QString::default(),
            revision: 0,
            days: 30,
            today_ms: 0,
            week_ms: 0,
            month_ms: 0,
            total_ms: 0,
            streak_days: 0,
            best_streak_days: 0,
            finished_total: 0,
            finished_month: 0,
            daily: QList::default(),
            max_daily_ms: 0,
            books: QList::default(),
        }
    }
}

impl cxx_qt::Initialize for qobject::StatsModel {
    fn initialize(self: Pin<&mut Self>) {
        let mut this = self;
        this.as_mut()
            .on_database_path_changed(|m| m.reload())
            .release();
        this.as_mut().on_revision_changed(|m| m.reload()).release();
        this.as_mut().on_days_changed(|m| m.reload()).release();
    }
}

struct Loaded {
    per_day: Vec<(Date, i64)>,
    today: Date,
    finished: Vec<Date>,
    books: Vec<stats::BookTotal>,
}

fn load(path: &str) -> audium_storage::Result<Option<Loaded>> {
    let db = ReadOnly::open(Path::new(path))?;
    let Some(today) = stats::today(db.conn())? else {
        return Ok(None);
    };
    Ok(Some(Loaded {
        per_day: stats::listening_per_day(db.conn())?,
        today,
        finished: stats::finished_dates(db.conn())?,
        books: stats::per_book_totals(db.conn(), 20)?,
    }))
}

fn to_i32(n: impl TryInto<i32>) -> i32 {
    n.try_into().unwrap_or(i32::MAX)
}

impl qobject::StatsModel {
    fn reload(mut self: Pin<&mut Self>) {
        let path = String::from(&self.database_path);
        if path.is_empty() {
            return;
        }
        let loaded = match load(&path) {
            Ok(Some(l)) => l,
            Ok(None) => return,
            Err(e) => {
                tracing::error!(error = %e, "cannot load statistics");
                return;
            }
        };
        let days = usize::try_from(self.days.clamp(1, 366)).unwrap_or(30);
        let s = summarize(&loaded.per_day, loaded.today, days);
        let daily: QList<QVariant> = s
            .daily
            .iter()
            .map(|(d, ms)| {
                QVariant::from(&variant_map(vec![
                    ("date", QVariant::from(&qs(&d.to_string()))),
                    ("year", QVariant::from(&d.year)),
                    ("month", QVariant::from(&to_i32(d.month))),
                    ("day", QVariant::from(&to_i32(d.day))),
                    ("weekday", QVariant::from(&to_i32(d.weekday()))),
                    ("ms", QVariant::from(ms)),
                ]))
            })
            .collect::<Vec<_>>()
            .into();
        let max_daily = s.daily.iter().map(|(_, ms)| *ms).max().unwrap_or(0);
        let books: QList<QVariant> = loaded
            .books
            .iter()
            .map(|b| {
                QVariant::from(&variant_map(vec![
                    ("bookId", QVariant::from(&b.book.0)),
                    ("title", QVariant::from(&qs(&b.title))),
                    (
                        "coverUrl",
                        QVariant::from(&file_url(b.cover_path.as_deref())),
                    ),
                    ("ms", QVariant::from(&b.listened_ms)),
                    ("sessions", QVariant::from(&b.sessions)),
                ]))
            })
            .collect::<Vec<_>>()
            .into();
        let month_start = loaded.today.month_start();
        let finished_month = loaded
            .finished
            .iter()
            .filter(|d| **d >= month_start && **d <= loaded.today)
            .count();

        set_ro!(self, today_ms, today_ms_changed, s.today_ms);
        set_ro!(self, week_ms, week_ms_changed, s.week_ms);
        set_ro!(self, month_ms, month_ms_changed, s.month_ms);
        set_ro!(self, total_ms, total_ms_changed, s.total_ms);
        set_ro!(
            self,
            streak_days,
            streak_days_changed,
            to_i32(s.streak_days)
        );
        set_ro!(
            self,
            best_streak_days,
            best_streak_days_changed,
            to_i32(s.best_streak_days)
        );
        set_ro!(
            self,
            finished_total,
            finished_total_changed,
            to_i32(loaded.finished.len())
        );
        set_ro!(
            self,
            finished_month,
            finished_month_changed,
            to_i32(finished_month)
        );
        set_ro!(self, daily, daily_changed, daily);
        set_ro!(self, max_daily_ms, max_daily_ms_changed, max_daily);
        set_ro!(self, books, books_changed, books);
    }
}

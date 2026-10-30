//! `BookDetails`: one book for the details page.

use std::path::Path;
use std::pin::Pin;

use audium_core::BookId;
use audium_storage::ReadOnly;
use audium_storage::read::{self, BookDetail, ChapterRow, TrackRow};
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QList, QMap, QMapPair_QString_QVariant, QString, QStringList, QUrl, QVariant};

use crate::convert::{file_url, qs, qs_opt, string_list};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
        include!("cxx-qt-lib/qlist.h");
        type QList_QVariant = cxx_qt_lib::QList<cxx_qt_lib::QVariant>;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, database_path, cxx_name = "databasePath")]
        #[qproperty(i32, revision)]
        #[qproperty(i64, book_id, cxx_name = "bookId")]
        #[qproperty(bool, valid, READ, NOTIFY)]
        #[qproperty(QString, title, READ, NOTIFY)]
        #[qproperty(QString, subtitle, READ, NOTIFY)]
        #[qproperty(QString, authors, READ, NOTIFY)]
        #[qproperty(QString, narrators, READ, NOTIFY)]
        #[qproperty(QString, series, READ, NOTIFY)]
        #[qproperty(QString, description, READ, NOTIFY)]
        #[qproperty(QString, genre, READ, NOTIFY)]
        #[qproperty(i32, year, READ, NOTIFY)]
        #[qproperty(i64, duration_ms, cxx_name = "durationMs", READ, NOTIFY)]
        #[qproperty(QUrl, cover_url, cxx_name = "coverUrl", READ, NOTIFY)]
        #[qproperty(bool, available, READ, NOTIFY)]
        /// Saved position (0 when never played).
        #[qproperty(i64, position_ms, cxx_name = "positionMs", READ, NOTIFY)]
        #[qproperty(bool, finished, READ, NOTIFY)]
        #[qproperty(i32, confidence, READ, NOTIFY)]
        #[qproperty(QStringList, review_reasons, cxx_name = "reviewReasons", READ, NOTIFY)]
        /// List of {title, durationMs, path, missing}.
        #[qproperty(QList_QVariant, tracks, READ, NOTIFY)]
        /// List of {title, startMs}.
        #[qproperty(QList_QVariant, chapters, READ, NOTIFY)]
        type BookDetails = super::BookDetailsRust;
    }

    impl cxx_qt::Initialize for BookDetails {}

    unsafe extern "RustQt" {
        #[qinvokable]
        fn reload(self: Pin<&mut BookDetails>);

    }
}

#[derive(Default)]
pub struct BookDetailsRust {
    database_path: QString,
    revision: i32,
    book_id: i64,
    valid: bool,
    title: QString,
    subtitle: QString,
    authors: QString,
    narrators: QString,
    series: QString,
    description: QString,
    genre: QString,
    year: i32,
    duration_ms: i64,
    cover_url: QUrl,
    available: bool,
    position_ms: i64,
    finished: bool,
    confidence: i32,
    review_reasons: QStringList,
    tracks: QList<QVariant>,
    chapters: QList<QVariant>,
}

impl cxx_qt::Initialize for qobject::BookDetails {
    fn initialize(self: Pin<&mut Self>) {
        let mut this = self;
        this.as_mut()
            .on_database_path_changed(|d| d.reload())
            .release();
        this.as_mut().on_revision_changed(|d| d.reload()).release();
        this.as_mut().on_book_id_changed(|d| d.reload()).release();
    }
}

fn map(entries: Vec<(&str, QVariant)>) -> QVariant {
    let mut m = QMap::<QMapPair_QString_QVariant>::default();
    for (k, v) in entries {
        m.insert(qs(k), v);
    }
    QVariant::from(&m)
}

type Loaded = (BookDetail, Vec<TrackRow>, Vec<ChapterRow>);

fn load(path: &str, id: i64) -> Result<Option<Loaded>, audium_storage::StorageError> {
    if path.is_empty() || id <= 0 {
        return Ok(None);
    }
    let db = ReadOnly::open(Path::new(path))?;
    let id = BookId(id);
    let Some(detail) = read::book(db.conn(), id)? else {
        return Ok(None);
    };
    Ok(Some((
        detail,
        read::tracks(db.conn(), id)?,
        read::chapters(db.conn(), id)?,
    )))
}

impl qobject::BookDetails {
    fn reload(mut self: Pin<&mut Self>) {
        let loaded = load(&String::from(&self.database_path), self.book_id).unwrap_or_else(|e| {
            tracing::error!(error = %e, "cannot load book");
            None
        });
        let Some((d, tracks, chapters)) = loaded else {
            set_ro!(self, valid, valid_changed, false);
            return;
        };
        let s = &d.summary;
        let series = s.series.as_ref().map(|name| match &s.series_index {
            Some(i) => format!("{name} #{i}"),
            None => name.clone(),
        });

        let track_list: QList<QVariant> = tracks
            .iter()
            .map(|t| {
                map(vec![
                    (
                        "title",
                        QVariant::from(&qs(t.title.as_deref().unwrap_or(&t.display_path))),
                    ),
                    (
                        "fileName",
                        QVariant::from(&qs(&t
                            .path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default())),
                    ),
                    ("path", QVariant::from(&qs(&t.display_path))),
                    ("durationMs", QVariant::from(&t.duration_ms)),
                    ("missing", QVariant::from(&t.missing)),
                ])
            })
            .collect::<Vec<_>>()
            .into();
        let chapter_list: QList<QVariant> = chapters
            .iter()
            .map(|c| {
                map(vec![
                    ("title", QVariant::from(&qs(&c.title))),
                    ("startMs", QVariant::from(&c.start_ms)),
                ])
            })
            .collect::<Vec<_>>()
            .into();

        set_ro!(self, title, title_changed, qs(&s.title));
        set_ro!(
            self,
            subtitle,
            subtitle_changed,
            qs_opt(d.subtitle.as_deref())
        );
        set_ro!(self, authors, authors_changed, qs(&s.authors.join(", ")));
        set_ro!(
            self,
            narrators,
            narrators_changed,
            qs(&s.narrators.join(", "))
        );
        set_ro!(self, series, series_changed, qs_opt(series.as_deref()));
        set_ro!(
            self,
            description,
            description_changed,
            qs_opt(d.description.as_deref())
        );
        set_ro!(self, genre, genre_changed, qs_opt(d.genre.as_deref()));
        set_ro!(self, year, year_changed, d.year.unwrap_or(0));
        set_ro!(self, duration_ms, duration_ms_changed, s.duration_ms);
        set_ro!(
            self,
            cover_url,
            cover_url_changed,
            file_url(s.cover_path.as_deref())
        );
        set_ro!(self, available, available_changed, s.available);
        set_ro!(self, position_ms, position_ms_changed, s.position_ms);
        set_ro!(self, finished, finished_changed, s.finished);
        set_ro!(
            self,
            confidence,
            confidence_changed,
            i32::from(s.confidence)
        );
        set_ro!(
            self,
            review_reasons,
            review_reasons_changed,
            string_list(&s.review_reasons)
        );
        set_ro!(self, tracks, tracks_changed, track_list);
        set_ro!(self, chapters, chapters_changed, chapter_list);
        set_ro!(self, valid, valid_changed, true);
    }
}

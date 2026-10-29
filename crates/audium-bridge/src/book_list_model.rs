//! `BookListModel`: the books of the library for grid and list views.
//!
//! Bind `databasePath` and `revision` to the `LibraryController`; the model
//! reloads whenever either (or `filter`) changes.

use std::path::Path;
use std::pin::Pin;

use audium_storage::ReadOnly;
use audium_storage::read::{self, BookFilter, BookSummary};
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

use crate::convert::{file_url, qs, qs_opt, string_list};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!(<QtCore/QAbstractListModel>);
        type QAbstractListModel;

        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(QString, database_path, cxx_name = "databasePath")]
        #[qproperty(i32, revision)]
        /// 0 = all books, 1 = "Needs review".
        #[qproperty(i32, filter)]
        #[qproperty(i32, count, READ, NOTIFY)]
        #[qproperty(QString, error_string, cxx_name = "errorString", READ, NOTIFY)]
        type BookListModel = super::BookListModelRust;
    }

    impl cxx_qt::Initialize for BookListModel {}

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut BookListModel>);

        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut BookListModel>);
    }

    unsafe extern "RustQt" {
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &BookListModel, parent: &QModelIndex) -> i32;

        #[cxx_override]
        fn data(self: &BookListModel, index: &QModelIndex, role: i32) -> QVariant;

        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &BookListModel) -> QHash_i32_QByteArray;

        #[qinvokable]
        fn reload(self: Pin<&mut BookListModel>);
    }
}

#[derive(Default)]
pub struct BookListModelRust {
    database_path: QString,
    revision: i32,
    filter: i32,
    count: i32,
    error_string: QString,
    rows: Vec<BookSummary>,
}

// Qt::UserRole + n
const USER_ROLE: i32 = 0x0100;
const ROLES: &[&str] = &[
    "bookId",
    "title",
    "authors",
    "narrators",
    "series",
    "durationMs",
    "coverUrl",
    "available",
    "confidence",
    "trackCount",
    "needsReview",
    "reviewReasons",
];

impl cxx_qt::Initialize for qobject::BookListModel {
    fn initialize(self: Pin<&mut Self>) {
        let mut this = self;
        this.as_mut()
            .on_database_path_changed(|m| m.reload())
            .release();
        this.as_mut().on_revision_changed(|m| m.reload()).release();
        this.as_mut().on_filter_changed(|m| m.reload()).release();
    }
}

fn series_text(b: &BookSummary) -> Option<String> {
    let name = b.series.as_deref()?;
    Some(match b.series_index.as_deref() {
        Some(i) => format!("{name} #{i}"),
        None => name.to_owned(),
    })
}

impl qobject::BookListModel {
    fn reload(mut self: Pin<&mut Self>) {
        let path = String::from(&self.database_path);
        let filter = if self.filter == 1 {
            BookFilter::NeedsReview
        } else {
            BookFilter::All
        };
        let result = if path.is_empty() {
            Ok(Vec::new())
        } else {
            ReadOnly::open(Path::new(&path)).and_then(|db| read::list_books(db.conn(), filter))
        };
        let rows = match result {
            Ok(rows) => {
                set_ro!(self, error_string, error_string_changed, QString::default());
                rows
            }
            Err(e) => {
                tracing::error!(error = %e, "cannot list books");
                set_ro!(self, error_string, error_string_changed, qs(&e.to_string()));
                Vec::new()
            }
        };
        self.as_mut().begin_reset_model();
        let count = i32::try_from(rows.len()).unwrap_or(i32::MAX);
        self.as_mut().rust_mut().rows = rows;
        self.as_mut().end_reset_model();
        set_ro!(self, count, count_changed, count);
    }

    fn row_count(&self, parent: &QModelIndex) -> i32 {
        if parent.is_valid() { 0 } else { self.count }
    }

    fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(b) = usize::try_from(index.row())
            .ok()
            .and_then(|r| self.rows.get(r))
        else {
            return QVariant::default();
        };
        match role - USER_ROLE - 1 {
            0 => QVariant::from(&b.id.0),
            1 => QVariant::from(&qs(&b.title)),
            2 => QVariant::from(&qs(&b.authors.join(", "))),
            3 => QVariant::from(&qs(&b.narrators.join(", "))),
            4 => QVariant::from(&qs_opt(series_text(b).as_deref())),
            5 => QVariant::from(&b.duration_ms),
            6 => QVariant::from(&file_url(b.cover_path.as_deref())),
            7 => QVariant::from(&b.available),
            8 => QVariant::from(&i32::from(b.confidence)),
            9 => QVariant::from(&b.track_count),
            10 => QVariant::from(&!b.review_reasons.is_empty()),
            11 => QVariant::from(&string_list(&b.review_reasons)),
            _ => QVariant::default(),
        }
    }

    fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        for (i, name) in (1..).zip(ROLES) {
            roles.insert(USER_ROLE + i, QByteArray::from(*name));
        }
        roles
    }
}

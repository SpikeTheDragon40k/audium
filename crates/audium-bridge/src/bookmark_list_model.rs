//! `BookmarkListModel`: one book's bookmarks, in book order, with the name
//! of the chapter each falls in.

use std::path::Path;
use std::pin::Pin;

use audium_core::BookId;
use audium_storage::organize::{self, Bookmark};
use audium_storage::{ReadOnly, read};
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

use crate::convert::qs_opt;

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
        #[qproperty(i64, book_id, cxx_name = "bookId")]
        #[qproperty(i32, count, READ, NOTIFY)]
        type BookmarkListModel = super::BookmarkListModelRust;
    }

    impl cxx_qt::Initialize for BookmarkListModel {}

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut BookmarkListModel>);

        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut BookmarkListModel>);
    }

    unsafe extern "RustQt" {
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &BookmarkListModel, parent: &QModelIndex) -> i32;

        #[cxx_override]
        fn data(self: &BookmarkListModel, index: &QModelIndex, role: i32) -> QVariant;

        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &BookmarkListModel) -> QHash_i32_QByteArray;

        #[qinvokable]
        fn reload(self: Pin<&mut BookmarkListModel>);
    }
}

#[derive(Default)]
pub struct BookmarkListModelRust {
    database_path: QString,
    revision: i32,
    book_id: i64,
    count: i32,
    rows: Vec<(Bookmark, Option<String>)>,
}

const USER_ROLE: i32 = 0x0100;
const ROLES: &[&str] = &[
    "bookmarkId",
    "title",
    "note",
    "positionMs",
    "chapterTitle",
    "createdAt",
];

impl cxx_qt::Initialize for qobject::BookmarkListModel {
    fn initialize(self: Pin<&mut Self>) {
        let mut this = self;
        this.as_mut()
            .on_database_path_changed(|m| m.reload())
            .release();
        this.as_mut().on_revision_changed(|m| m.reload()).release();
        this.as_mut().on_book_id_changed(|m| m.reload()).release();
    }
}

/// Bookmarks with the title of the chapter containing each.
fn load(path: &str, book: BookId) -> audium_storage::Result<Vec<(Bookmark, Option<String>)>> {
    let db = ReadOnly::open(Path::new(path))?;
    let chapters = read::chapters(db.conn(), book)?;
    Ok(organize::bookmarks(db.conn(), book)?
        .into_iter()
        .map(|b| {
            let chapter = chapters
                .iter()
                .rev()
                .find(|c| c.start_ms <= b.book_position_ms)
                .map(|c| c.title.clone());
            (b, chapter)
        })
        .collect())
}

impl qobject::BookmarkListModel {
    fn reload(mut self: Pin<&mut Self>) {
        let path = String::from(&self.database_path);
        let rows = if path.is_empty() || self.book_id <= 0 {
            Vec::new()
        } else {
            load(&path, BookId(self.book_id)).unwrap_or_else(|e| {
                tracing::error!(error = %e, "cannot list bookmarks");
                Vec::new()
            })
        };
        let count = i32::try_from(rows.len()).unwrap_or(i32::MAX);
        self.as_mut().begin_reset_model();
        self.as_mut().rust_mut().rows = rows;
        self.as_mut().end_reset_model();
        set_ro!(self, count, count_changed, count);
    }

    fn row_count(&self, parent: &QModelIndex) -> i32 {
        // The rows themselves, not `count`: views re-read the row count
        // during endResetModel(), before `count` is updated.
        if parent.is_valid() {
            0
        } else {
            i32::try_from(self.rows.len()).unwrap_or(i32::MAX)
        }
    }

    fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some((b, chapter)) = usize::try_from(index.row())
            .ok()
            .and_then(|r| self.rows.get(r))
        else {
            return QVariant::default();
        };
        match role - USER_ROLE - 1 {
            0 => QVariant::from(&b.id),
            1 => QVariant::from(&qs_opt(b.title.as_deref())),
            2 => QVariant::from(&qs_opt(b.note.as_deref())),
            3 => QVariant::from(&b.book_position_ms),
            4 => QVariant::from(&qs_opt(chapter.as_deref())),
            5 => QVariant::from(&b.created_at),
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

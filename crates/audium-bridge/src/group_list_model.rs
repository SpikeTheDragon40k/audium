//! `GroupListModel`: authors, narrators, series or collections with their
//! number of books, for the browse pages and the bookshelf.

use std::path::Path;
use std::pin::Pin;

use audium_storage::ReadOnly;
use audium_storage::read::{self, GroupKind, GroupRow};
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

use crate::convert::{file_url, qs};

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
        /// 0 authors, 1 narrators, 2 series, 3 collections.
        #[qproperty(i32, kind)]
        #[qproperty(i32, count, READ, NOTIFY)]
        type GroupListModel = super::GroupListModelRust;
    }

    impl cxx_qt::Initialize for GroupListModel {}

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        fn begin_reset_model(self: Pin<&mut GroupListModel>);

        #[inherit]
        #[cxx_name = "endResetModel"]
        fn end_reset_model(self: Pin<&mut GroupListModel>);
    }

    unsafe extern "RustQt" {
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &GroupListModel, parent: &QModelIndex) -> i32;

        #[cxx_override]
        fn data(self: &GroupListModel, index: &QModelIndex, role: i32) -> QVariant;

        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &GroupListModel) -> QHash_i32_QByteArray;

        #[qinvokable]
        fn reload(self: Pin<&mut GroupListModel>);
    }
}

#[derive(Default)]
pub struct GroupListModelRust {
    database_path: QString,
    revision: i32,
    kind: i32,
    count: i32,
    rows: Vec<GroupRow>,
}

const USER_ROLE: i32 = 0x0100;
const ROLES: &[&str] = &["groupId", "name", "bookCount", "coverUrl"];

impl cxx_qt::Initialize for qobject::GroupListModel {
    fn initialize(self: Pin<&mut Self>) {
        let mut this = self;
        this.as_mut()
            .on_database_path_changed(|m| m.reload())
            .release();
        this.as_mut().on_revision_changed(|m| m.reload()).release();
        this.as_mut().on_kind_changed(|m| m.reload()).release();
    }
}

impl qobject::GroupListModel {
    fn reload(mut self: Pin<&mut Self>) {
        let path = String::from(&self.database_path);
        let kind = match self.kind {
            1 => GroupKind::Narrators,
            2 => GroupKind::Series,
            3 => GroupKind::Collections,
            _ => GroupKind::Authors,
        };
        let rows = if path.is_empty() {
            Vec::new()
        } else {
            ReadOnly::open(Path::new(&path))
                .and_then(|db| read::list_groups(db.conn(), kind))
                .unwrap_or_else(|e| {
                    tracing::error!(error = %e, "cannot list groups");
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
        let Some(g) = usize::try_from(index.row())
            .ok()
            .and_then(|r| self.rows.get(r))
        else {
            return QVariant::default();
        };
        match role - USER_ROLE - 1 {
            0 => QVariant::from(&g.id),
            1 => QVariant::from(&qs(&g.name)),
            2 => QVariant::from(&g.book_count),
            3 => QVariant::from(&file_url(g.cover_path.as_deref())),
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

//! CXX-Qt objects and list models exposed to QML.
//!
//! QML talks only to the types in this crate. They run no SQL themselves
//! (they call `audium-storage`) and never touch QObjects from worker threads:
//! background work posts its results through `CxxQtThread::queue`.

/// Sets a read-only (`READ, NOTIFY`) property of a CXX-Qt object and emits
/// its change signal if the value changed:
/// `set_ro!(obj, field, field_changed, value)`.
macro_rules! set_ro {
    ($obj:expr, $field:ident, $signal:ident, $value:expr) => {{
        let value = $value;
        if $obj.$field != value {
            $obj.as_mut().rust_mut().$field = value;
            $obj.as_mut().$signal();
        }
    }};
}

pub mod app_info;
pub mod book_details;
pub mod book_list_model;
mod convert;
pub mod library_controller;
pub mod playback_session;

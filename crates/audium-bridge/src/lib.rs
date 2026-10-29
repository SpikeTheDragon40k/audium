//! CXX-Qt objects and list models exposed to QML.
//!
//! QML talks only to the types in this crate. They hold no SQL and never touch
//! QObjects from worker threads.

pub mod app_info;

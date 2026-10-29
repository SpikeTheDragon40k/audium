//! `AppInfo`: static application information for QML, and the Milestone 0 bridge check.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(QString, version, READ, CONSTANT)]
        #[qproperty(QString, app_id, cxx_name = "appId", READ, CONSTANT)]
        type AppInfo = super::AppInfoRust;

        /// Returns `text` unchanged after a round trip through Rust.
        #[qinvokable]
        fn echo(self: &AppInfo, text: &QString) -> QString;
    }
}

use cxx_qt_lib::QString;

pub struct AppInfoRust {
    version: QString,
    app_id: QString,
}

impl Default for AppInfoRust {
    fn default() -> Self {
        Self {
            version: QString::from(audium_core::VERSION),
            app_id: QString::from(audium_core::APP_ID),
        }
    }
}

impl qobject::AppInfo {
    fn echo(&self, text: &QString) -> QString {
        QString::from(&String::from(text))
    }
}

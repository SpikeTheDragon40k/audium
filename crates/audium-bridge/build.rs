use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("org.audium.bridge"))
        .files(["src/app_info.rs"])
        .build();
}

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("org.audium.bridge"))
        .files([
            "src/app_info.rs",
            "src/library_controller.rs",
            "src/book_list_model.rs",
            "src/book_details.rs",
            "src/playback_session.rs",
        ])
        .build();
}

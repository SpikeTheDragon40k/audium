//! A thread that owns a read-write connection and runs writes in order, so
//! frequent small writes (progress) never block the UI thread, not even while
//! a scan holds the database's write lock.

use std::path::Path;
use std::sync::mpsc;
use std::thread::JoinHandle;

use crate::{Database, Result};

type Job = Box<dyn FnOnce(&mut Database) + Send>;

pub struct Writer {
    tx: Option<mpsc::Sender<Job>>,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    /// Opens the database (on the calling thread, so errors are reported
    /// here) and starts the writer thread.
    pub fn open(path: &Path) -> Result<Self> {
        let mut db = Database::open(path)?;
        let (tx, rx) = mpsc::channel::<Job>();
        let thread = std::thread::Builder::new()
            .name("audium-db-writer".into())
            .spawn(move || {
                for job in rx {
                    job(&mut db);
                }
            })
            .map_err(|e| {
                crate::StorageError::Invalid(format!("cannot start the database writer: {e}"))
            })?;
        Ok(Writer {
            tx: Some(tx),
            thread: Some(thread),
        })
    }

    /// Queues a write. Errors inside it should be logged by the job itself.
    pub fn submit(&self, job: impl FnOnce(&mut Database) + Send + 'static) {
        if let Some(tx) = &self.tx
            && tx.send(Box::new(job)).is_err()
        {
            tracing::error!("database writer has stopped; a write was lost");
        }
    }

    /// Waits until every queued write is done.
    pub fn flush(&self) {
        let (done_tx, done_rx) = mpsc::channel();
        self.submit(move |_| {
            let _ = done_tx.send(());
        });
        let _ = done_rx.recv();
    }
}

impl Drop for Writer {
    /// Finishes the queued writes before returning.
    fn drop(&mut self) {
        drop(self.tx.take());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

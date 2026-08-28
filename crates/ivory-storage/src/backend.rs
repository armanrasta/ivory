//! Thin RocksDB wrapper.

use std::path::Path;

use rocksdb::{DB, Options};
use thiserror::Error;

/// Errors from the RocksDB backend.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Underlying RocksDB error.
    #[error(transparent)]
    RocksDb(#[from] rocksdb::Error),
}

/// RocksDB-backed key-value store.
pub struct RocksDbBackend {
    db: DB,
}

impl RocksDbBackend {
    /// Open (or create) a database at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::RocksDb`] if RocksDB cannot open the path.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path)?;
        Ok(Self { db })
    }

    /// Read a key. Missing keys return `None`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::RocksDb`] on a database read failure.
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(self.db.get(key)?)
    }

    /// Write a key.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::RocksDb`] on a database write failure.
    pub fn put(&self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        self.db.put(key, value)?;
        Ok(())
    }

    /// Delete a key.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::RocksDb`] on a database delete failure.
    pub fn delete(&self, key: &[u8]) -> Result<(), StorageError> {
        self.db.delete(key)?;
        Ok(())
    }

    /// Flush memtables to disk.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::RocksDb`] if the flush fails.
    pub fn flush(&self) -> Result<(), StorageError> {
        self.db.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_temp() -> (tempfile::TempDir, RocksDbBackend) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = RocksDbBackend::open(dir.path()).expect("open rocksdb");
        (dir, db)
    }

    #[test]
    fn put_get_roundtrip() {
        let (_dir, db) = open_temp();
        db.put(b"key", b"value").unwrap();
        assert_eq!(db.get(b"key").unwrap().as_deref(), Some(b"value".as_ref()));
    }

    #[test]
    fn missing_key_returns_none() {
        let (_dir, db) = open_temp();
        assert_eq!(db.get(b"missing").unwrap(), None);
    }

    #[test]
    fn delete_then_get_returns_none() {
        let (_dir, db) = open_temp();
        db.put(b"key", b"value").unwrap();
        db.delete(b"key").unwrap();
        assert_eq!(db.get(b"key").unwrap(), None);
    }

    #[test]
    fn overwrite_value() {
        let (_dir, db) = open_temp();
        db.put(b"key", b"one").unwrap();
        db.put(b"key", b"two").unwrap();
        assert_eq!(db.get(b"key").unwrap().as_deref(), Some(b"two".as_ref()));
    }

    #[test]
    fn flush_succeeds() {
        let (_dir, db) = open_temp();
        db.put(b"key", b"value").unwrap();
        db.flush().unwrap();
        assert_eq!(db.get(b"key").unwrap().as_deref(), Some(b"value".as_ref()));
    }
}

// Copyright 2024 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: GPL-3.0-only

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs::{self, File},
    path::PathBuf,
};

use super::{MnemonicStorage, MnemonicStorageError, StoredMnemonic};

#[derive(Debug, thiserror::Error)]
pub enum OnDiskMnemonicStorageError {
    #[error("mnemonic already stored")]
    MnemonicAlreadyStored { path: PathBuf },

    #[error("failed to create file")]
    FileCreateError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to open file")]
    FileOpenError(#[source] std::io::Error),

    #[error("failed to read mnemonic from file")]
    ReadError(#[source] serde_json::Error),

    #[error("failed to write mnemonic to file")]
    WriteError(#[source] serde_json::Error),

    #[error("failed to remove mnemonic file")]
    RemoveError(#[source] std::io::Error),
}

impl MnemonicStorageError for OnDiskMnemonicStorageError {
    fn is_mnemonic_stored(&self) -> bool {
        matches!(
            self,
            OnDiskMnemonicStorageError::MnemonicAlreadyStored { .. }
        )
    }
}

pub struct OnDiskMnemonicStorage {
    path: PathBuf,
}

impl OnDiskMnemonicStorage {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl MnemonicStorage for OnDiskMnemonicStorage {
    type StorageError = OnDiskMnemonicStorageError;

    async fn store_mnemonic(
        &self,
        mnemonic: bip39::Mnemonic,
    ) -> Result<(), OnDiskMnemonicStorageError> {
        let name = "default".to_string();
        let nonce = 0;
        let stored_mnemonic = StoredMnemonic {
            name,
            mnemonic,
            nonce,
        };

        // Error if the file already exists
        if self.path.exists() {
            return Err(OnDiskMnemonicStorageError::MnemonicAlreadyStored {
                path: self.path.clone(),
            });
        }

        // Another layer of defense, only create the file if it doesn't already exist
        let file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&self.path)
            .map_err(|err| OnDiskMnemonicStorageError::FileCreateError {
                path: self.path.clone(),
                source: err,
            })?;

        // Create parent directories
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                OnDiskMnemonicStorageError::FileCreateError {
                    path: parent.to_path_buf(),
                    source: err,
                }
            })?;

            #[cfg(unix)]
            {
                // Set directory permissions to 700 (rwx------)
                let permissions = fs::Permissions::from_mode(0o700);
                fs::set_permissions(parent, permissions).map_err(|source| {
                    OnDiskMnemonicStorageError::FileCreateError {
                        path: parent.to_path_buf(),
                        source,
                    }
                })?;
            }

            // TODO: same for windows
        }

        serde_json::to_writer(file, &stored_mnemonic)
            .map_err(OnDiskMnemonicStorageError::WriteError)?;

        #[cfg(unix)]
        {
            // Set directory permissions to 600 (rw------)
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(self.path.clone(), permissions).map_err(|source| {
                OnDiskMnemonicStorageError::FileCreateError {
                    path: self.path.clone(),
                    source,
                }
            })?;
        }

        // TODO: same for windows

        Ok(())
    }

    async fn load_mnemonic(&self) -> Result<bip39::Mnemonic, OnDiskMnemonicStorageError> {
        tracing::debug!("Opening: {}", self.path.display());

        // Make sure that the file has permissions set to 600 (rw------)
        #[cfg(unix)]
        {
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&self.path, permissions)
                .map_err(OnDiskMnemonicStorageError::FileOpenError)?;
        }

        let file = File::open(&self.path).map_err(OnDiskMnemonicStorageError::FileOpenError)?;
        serde_json::from_reader(file)
            .map_err(OnDiskMnemonicStorageError::ReadError)
            .map(|s: StoredMnemonic| s.mnemonic.clone())
    }

    async fn remove_mnemonic(&self) -> Result<(), OnDiskMnemonicStorageError> {
        std::fs::remove_file(&self.path).map_err(OnDiskMnemonicStorageError::RemoveError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn store_mnemonic() {
        let mnemonic = bip39::Mnemonic::generate_in(bip39::Language::English, 12).unwrap();
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("test.txt");
        let mnemonic_storage = OnDiskMnemonicStorage::new(path.clone());
        mnemonic_storage
            .store_mnemonic(mnemonic.clone())
            .await
            .unwrap();

        let stored_mnemonic = mnemonic_storage.load_mnemonic().await.unwrap();
        assert_eq!(mnemonic, stored_mnemonic);
    }

    #[tokio::test]
    async fn store_twice_fails() {
        let mnemonic = bip39::Mnemonic::generate_in(bip39::Language::English, 12).unwrap();
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("test.txt");
        let mnemonic_storage = OnDiskMnemonicStorage::new(path.clone());
        mnemonic_storage
            .store_mnemonic(mnemonic.clone())
            .await
            .unwrap();

        let result = mnemonic_storage.store_mnemonic(mnemonic).await;
        assert!(matches!(
            result,
            Err(OnDiskMnemonicStorageError::MnemonicAlreadyStored { .. })
        ));
    }

    #[tokio::test]
    async fn load_fails_if_file_does_not_exist() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("test.txt");
        let mnemonic_storage = OnDiskMnemonicStorage::new(path.clone());
        let result = mnemonic_storage.load_mnemonic().await;
        assert!(matches!(
            result,
            Err(OnDiskMnemonicStorageError::FileOpenError(_))
        ));
    }

    #[tokio::test]
    async fn load_fails_if_no_mnemonic_file() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("test.txt");
        let mnemonic_storage = OnDiskMnemonicStorage::new(path.clone());
        let result = mnemonic_storage.load_mnemonic().await;
        assert!(matches!(
            result,
            Err(OnDiskMnemonicStorageError::FileOpenError(_))
        ));
    }

    #[tokio::test]
    async fn load_fails_if_no_mnemonic_stored() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("test.txt");
        let mnemonic_storage = OnDiskMnemonicStorage::new(path.clone());
        let _ = File::create(&path).unwrap();
        let result = mnemonic_storage.load_mnemonic().await;
        assert!(matches!(
            result,
            Err(OnDiskMnemonicStorageError::ReadError(_))
        ));
    }
}

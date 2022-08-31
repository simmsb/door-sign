use std::sync::Mutex;

use embedded_svc::storage::{RawStorage, Storage, StorageBase};
use esp_idf_svc::nvs_storage::EspNvsStorage;
use once_cell::sync::OnceCell;

pub static STORAGE: OnceCell<Mutex<PostcardStorage>> = OnceCell::new();

pub struct PostcardStorage(EspNvsStorage);

impl StorageBase for PostcardStorage {
    type Error = color_eyre::Report;

    fn contains(&self, name: &str) -> Result<bool, Self::Error> {
        self.0.contains(name).map_err(|e| e.into())
    }

    fn remove(&mut self, name: &str) -> Result<bool, Self::Error> {
        self.0.remove(name).map_err(|e| e.into())
    }
}

impl Storage for PostcardStorage {
    fn get<T>(&self, name: &str) -> Result<Option<T>, Self::Error>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut buf = [0u8; 256];

        if let Some((buf, _)) = self.0.get_raw(name, &mut buf)? {
            Ok(Some(postcard::from_bytes(buf)?))
        } else {
            Ok(None)
        }
    }

    fn set<T>(&mut self, name: &str, value: &T) -> Result<bool, Self::Error>
    where
        T: serde::Serialize,
    {
        let mut buf = [0u8; 256];

        let buf = postcard::to_slice(value, &mut buf)?;

        self.0.put_raw(name, buf).map_err(|e| e.into())
    }
}

pub fn init(s: EspNvsStorage) {
    let _ = STORAGE.set(Mutex::new(PostcardStorage(s)));
}

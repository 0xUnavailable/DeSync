pub mod object_store;
pub mod organized_store;
pub mod log_index;

pub use object_store::ObjectStore;
pub use organized_store::{OrganizedObjectStore, ObjectType, ObjectStoreStats};
pub use log_index::{LogVersionIndex, build_index_from_store};

use anyhow::Result;
use crate::types::HASH;

pub trait ObjectStoreInterface {
    fn list_all(&self) -> Result<Vec<HASH>>;
    fn get_object(&self, hash: &HASH) -> Result<Option<Vec<u8>>>;
    fn store_object(&self, hash: &HASH, bytes: &[u8]) -> Result<()>;
    fn has_object(&self, hash: &HASH) -> bool;
}
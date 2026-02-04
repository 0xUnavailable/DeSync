use realm::storage::ObjectStore;
use std::fs;

fn main() -> anyhow::Result<()> {
    let genesis_bytes = fs::read("genesis_log.json")?;
    let hash = blake3::hash(&genesis_bytes);
    
    let store = ObjectStore::new("./objects")?;
    store.store_object(hash.as_bytes(), &genesis_bytes)?;
    
    println!("Genesis log loaded into object store");
    Ok(())
}
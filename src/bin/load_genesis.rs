use std::fs;
use realm::storage::OrganizedObjectStore;

fn main() -> anyhow::Result<()> {
    let genesis_bytes = fs::read("genesis_log.json")?;
    let hash = blake3::hash(&genesis_bytes);
    
    let store = OrganizedObjectStore::new("./objects")?;
    store.store_object(hash.as_bytes(), &genesis_bytes)?;
    
    println!("Genesis log loaded into object store");
    println!("  Hash: {}", hex::encode(hash.as_bytes()));
    println!("  Stored in: objects/00_government_logs/");
    Ok(())
}
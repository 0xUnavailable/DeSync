use serde::Serialize;
use ciborium::from_reader;
use ciborium::into_writer;
use serde::de::DeserializeOwned;

pub fn serialize<T: Serialize>(data: &T) -> Result<Vec<u8>, anyhow::Error> {
    let mut bytes = Vec::new();
    into_writer(data, &mut bytes)?;

    Ok(bytes)
}

pub fn deserialize<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, anyhow::Error> {
    let data: T = from_reader(bytes)?;

    Ok(data)
}

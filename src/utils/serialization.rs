pub fn serialize<T: serde::Serialize>(_obj: &T) -> Vec<u8> {
    // TODO: implement deterministic serialization
    vec![]
}

pub fn deserialize<T: serde::de::DeserializeOwned>(_bytes: &[u8]) -> T {
    // TODO: implement deserialization
    unimplemented!()
}

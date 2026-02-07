// Simple VDF implementation using iterative hashing
// This provides proof-of-sequential-work without external dependencies

use serde::{Serialize, Deserialize};
use blake3;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct VDFProof {
    pub challenge: Vec<u8>,
    pub iterations: u64,
    pub result: [u8; 32],
}

impl VDFProof {
    pub fn compute(challenge: &[u8], iterations: u64) -> Self {
        let mut current = blake3::hash(challenge);
        
        for _ in 0..iterations {
            current = blake3::hash(current.as_bytes());
        }
        
        Self {
            challenge: challenge.to_vec(),
            iterations,
            result: *current.as_bytes(),
        }
    }
    
    pub fn verify(&self) -> bool {
        let mut current = blake3::hash(&self.challenge);
        
        for _ in 0..self.iterations {
            current = blake3::hash(current.as_bytes());
        }
        
        *current.as_bytes() == self.result
    }
}



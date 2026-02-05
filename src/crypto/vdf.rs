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
    /// Compute VDF by performing sequential hash iterations
    /// This simulates actual work and cannot be parallelized
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
    
    /// Verify VDF by re-computing the sequential hashing
    pub fn verify(&self) -> bool {
        let mut current = blake3::hash(&self.challenge);
        
        for _ in 0..self.iterations {
            current = blake3::hash(current.as_bytes());
        }
        
        *current.as_bytes() == self.result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_vdf_compute_and_verify() {
        let challenge = b"test_challenge";
        let proof = VDFProof::compute(challenge, 1000);
        
        assert_eq!(proof.challenge, challenge);
        assert_eq!(proof.iterations, 1000);
        assert!(proof.verify());
    }
    
    #[test]
    fn test_vdf_invalid_proof() {
        let challenge = b"test_challenge";
        let mut proof = VDFProof::compute(challenge, 1000);
        
        // Tamper with result
        proof.result[0] ^= 1;
        
        assert!(!proof.verify());
    }
}


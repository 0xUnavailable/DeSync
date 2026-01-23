use vdf::{VDFParams, WesolowskiVDFParams, VDF};
use serde::{Serialize, Deserialize};
use std::panic;


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct VDFProof {
    pub challenge: Vec<u8>,
    pub difficulty: u64,
    pub solution: Vec<u8>,
}

impl VDFProof {
    pub fn compute(challenge: &[u8], difficulty: u64) -> Self {
        let vdf_instance = WesolowskiVDFParams(2048).new();
        let solution = vdf_instance.solve(challenge, difficulty)
        .expect("VDF computation failed");
        Self {
            challenge: challenge.to_vec(),
            difficulty,
            solution,
        }
    }
    pub fn verify(&self) -> bool {
    let vdf_instance = WesolowskiVDFParams(2048).new();
    
    // Catch panics from vdf crate
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        vdf_instance.verify(&self.challenge, self.difficulty, &self.solution)
    }));
    
    match result {
        Ok(Ok(())) => true,
        Ok(Err(_)) => false,
        Err(_) => false, 
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_real_vdf_performance() {
        let challenge = b"network-checkpoint-alpha";
        let difficulty = 5000;

        let start = std::time::Instant::now();
        let proof = VDFProof::compute(challenge, difficulty);
        let duration_solve = start.elapsed();

        let start_ver = std::time::Instant::now();
        let is_valid = proof.verify();
        let duration_verify = start_ver.elapsed();

        println!("\n--- VDF PERFORMANCE ---");
        println!("Solve Time:    {:?}", duration_solve);
        println!("Verify Time:   {:?}", duration_verify);
        println!("Proof Size:    {} bytes", proof.solution.len());
        
        assert!(is_valid, "VDF proof must verify correctly");
        
        let proof_2 = VDFProof::compute(challenge, difficulty);
        assert_eq!(proof.solution, proof_2.solution, "VDF must be deterministic!");
    }

    #[test]
    fn test_vdf_wrong_solution_fails() {
        // Suppress panic output during this test
        let _ = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        
        let challenge_a = b"challenge-a";
        let challenge_b = b"challenge-b";
        let difficulty = 500;

        let proof = VDFProof::compute(challenge_a, difficulty);
        
        assert!(
            !VDFProof {
                challenge: challenge_b.to_vec(),
                ..proof
            }.verify(),
            "VDF must fail when solution is for a different challenge"
        );
        
        // Restore default panic hook
        let _ = std::panic::take_hook();
    }
    #[test]
    fn test_vdf_different_challenges() {
        let challenge1 = b"alpha";
        let challenge2 = b"beta";
        let difficulty = 200;

        let proof1 = VDFProof::compute(challenge1, difficulty);
        let proof2 = VDFProof::compute(challenge2, difficulty);

        assert_ne!(proof1.solution, proof2.solution, "Different challenges must produce different outputs");
    }

    #[test]
    fn test_vdf_different_difficulty() {
        let challenge = b"same-challenge";
        let difficulty1 = 1000;
        let difficulty2 = 2000;

        let proof1 = VDFProof::compute(challenge, difficulty1);
        let proof2 = VDFProof::compute(challenge, difficulty2);

        assert_ne!(proof1.solution, proof2.solution, "Different difficulty must produce different solutions");
    }

    #[test]
    fn test_vdf_zero_difficulty() {
        let challenge = b"zero-difficulty";
        let difficulty = 0;

        let proof = VDFProof::compute(challenge, difficulty);
        assert!(proof.verify(), "VDF with zero difficulty should verify correctly");
    }
}

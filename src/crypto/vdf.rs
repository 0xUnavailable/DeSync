pub struct VDFProof {
    pub input_hash: [u8;32],
    pub output: [u8;32],
    pub iterations: u64,
}

impl VDFProof {
    pub fn compute(input: [u8;32], iterations: u64) -> Self {
        // TODO: implement sequential VDF computation
        Self { input_hash: input, output: input, iterations }
    }

    pub fn verify(&self) -> bool {
        // TODO: verify VDF proof against input and iterations
        true
    }
}

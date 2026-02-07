use rand::rngs::OsRng;
use ed25519_dalek::{SigningKey, Signature, Signer, Verifier, VerifyingKey};

pub fn generate_keypair() -> SigningKey {
    let mut csprng = OsRng;
    SigningKey::generate(&mut csprng)
}

pub fn sign_message(signing_key: &SigningKey, message: &[u8]) -> Signature {
    signing_key.sign(message)
}

pub fn verify_signature(pubkey: &VerifyingKey, message: &[u8], signature: &Signature) -> bool {
    pubkey.verify(message, signature).is_ok()
}



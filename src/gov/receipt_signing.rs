
use crate::gov::GovernmentNode;
use crate::types::{HASH, TaxObligation, TaxReceipt};
use crate::utils::serialize;
use anyhow::{Result, anyhow, bail};
use ed25519_dalek::{Signature, Signer, Verifier};

pub fn sign_tax_receipt(
    gov: &GovernmentNode,
    obligation: &TaxObligation,
    previous_receipt_hash: Option<HASH>,
    payment_slot: u64,
) -> Result<TaxReceipt> {
    let current_log = gov
        .latest_log
        .as_ref()
        .ok_or_else(|| anyhow!("No government log available"))?;

    let previous_log = gov.previous_log.as_ref();

    let is_current = obligation.government_log.log_hash == current_log.log_hash;
    let is_previous = previous_log
        .map(|prev| obligation.government_log.log_hash == prev.log_hash)
        .unwrap_or(false);

    if !is_current && !is_previous {
        bail!("Government log not recognized (not current or previous)");
    }

    let mut unsigned = obligation.clone();
    unsigned.user_signature = [0; 64];
    unsigned.obligation_hash = [0; 32];

    let computed_hash: HASH = *blake3::hash(&serialize(&unsigned)?).as_bytes();

    if computed_hash != obligation.obligation_hash {
        return Err(anyhow!("Invalid obligation hash"));
    }

    let sig = Signature::from_bytes(&obligation.user_signature);
    obligation
        .user_id
        .verify(computed_hash.as_slice(), &sig)
        .map_err(|_| anyhow!("Invalid user signature"))?;

    let mut receipt = TaxReceipt {
        obligation_hash: obligation.obligation_hash,
        previous_receipt_hash,
        payment_slot,
        coins_transferred: obligation.coin_amount,
        government_signature: [0; 64],
    };

    let receipt_hash = blake3::hash(&serialize(&receipt)?);
    receipt.government_signature = gov.signing_key.sign(receipt_hash.as_bytes()).to_bytes();

    Ok(receipt)
}

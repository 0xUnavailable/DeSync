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
// #[cfg(test)]
// mod tax_tests {
//     use std::collections::BTreeMap;

//     use super::*;
//     use crate::types::{ActionRule, GovernmentRuleLog, HASH, RealmId, ResourceDelta, TaxObligation};
//     use crate::utils::serialize;
//     use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
//     use rand::rngs::OsRng;

//     fn setup_gov_log_multi(version: u64, tax_rate: u8, effective_slot: u64) -> GovernmentRuleLog {
//         let mut rules = BTreeMap::new();
//         for crop in ["wheat", "corn"] {
//             rules.insert(
//                 format!("plant:{}", crop),
//                 ActionRule {
//                     energy_cost: 10,
//                     resource_reward: ResourceDelta {
//                         wheat: 0,
//                         wood: 0,
//                         coins: 0,
//                     },
//                     maturation_iterations: 1_000,
//                 },
//             );
//             rules.insert(
//                 format!("harvest:{}", crop),
//                 ActionRule {
//                     energy_cost: 5,
//                     resource_reward: ResourceDelta {
//                         wheat: 50,
//                         wood: 0,
//                         coins: 20,
//                     },
//                     maturation_iterations: 1_000,
//                 },
//             );
//         }

//         let mut log = GovernmentRuleLog {
//             version_number: version,
//             effective_slot,
//             rules,
//             tasks: vec![],
//             tax_rate_percent: tax_rate,
//             tax_deadline: 100,
//             log_hash: [0; 32],
//             government_signature: [0; 64],
//             previous_log_hash: None,
//             realm_id: [0;32],
//         };

//         let mut unsigned_log = log.clone();
//         unsigned_log.log_hash = [0; 32];
//         unsigned_log.government_signature = [0; 64];
//         log.log_hash = *blake3::hash(&serialize(&unsigned_log).unwrap()).as_bytes();

//         log
//     }

//     fn setup_government_node(
//         current_log: GovernmentRuleLog,
//         previous_log: Option<GovernmentRuleLog>,
//     ) -> GovernmentNode {
//         let mut rng = OsRng;
//         GovernmentNode {
//             realm_id: [0;32],
//             signing_key: SigningKey::generate(&mut rng),
//             latest_log: Some(current_log),
//             previous_log,
//         }
//     }

//     fn build_tax_obligation(
//         user_key: &SigningKey,
//         gov_log: GovernmentRuleLog,
//         coin_amount: u64,
//         sequence_number: u64,
//         event_hash: HASH,
//         previous_receipt_hash: Option<HASH>,
//         realm_id: RealmId,
//     ) -> TaxObligation {
//         let mut obligation = TaxObligation {
//             realm_id,
//             event_hash,
//             user_id: VerifyingKey::from(user_key),
//             coin_amount,
//             government_log: gov_log,
//             sequence_number,
//             user_signature: [0; 64],
//             obligation_hash: [0; 32],
//             previous_receipt_hash,
//         };

//         let mut unsigned = obligation.clone();
//         unsigned.user_signature = [0; 64];
//         unsigned.obligation_hash = [0; 32];

//         let hash: HASH = *blake3::hash(&serialize(&unsigned).unwrap()).as_bytes();
//         obligation.obligation_hash = hash;
//         obligation.user_signature = user_key.sign(&hash).to_bytes();

//         obligation
//     }

//     #[test]
//     fn tax_obligation_happy_path() {
//         let gov_log = setup_gov_log_multi(1, 10, 0);
//         let gov = setup_government_node(gov_log.clone(), None);

//         let mut rng = OsRng;
//         let user_key = SigningKey::generate(&mut rng);

//         let obligation = build_tax_obligation(&user_key, gov_log, 20, 1, [1; 32], None);

//         let receipt = sign_tax_receipt(&gov, &obligation, None, 42).unwrap();

//         let mut unsigned_receipt = receipt.clone();
//         unsigned_receipt.government_signature = [0; 64];

//         let receipt_hash = blake3::hash(&serialize(&unsigned_receipt).unwrap());
//         let sig = Signature::from_bytes(&receipt.government_signature);

//         gov.signing_key
//             .verifying_key()
//             .verify(receipt_hash.as_bytes(), &sig)
//             .unwrap();

//         assert_eq!(receipt.coins_transferred, 20);
//         assert_eq!(receipt.obligation_hash, obligation.obligation_hash);
//         assert_eq!(receipt.previous_receipt_hash, None);
//         assert_eq!(receipt.payment_slot, 42);
//     }

//     #[test]
//     fn accepts_previous_government_log() {
//         let old_log = setup_gov_log_multi(1, 10, 0);
//         let new_log = setup_gov_log_multi(2, 15, 100);
//         let gov = setup_government_node(new_log.clone(), Some(old_log.clone()));

//         let mut rng = OsRng;
//         let user_key = SigningKey::generate(&mut rng);

//         let obligation = build_tax_obligation(&user_key, old_log, 15, 1, [2; 32], None);
//         let receipt = sign_tax_receipt(&gov, &obligation, None, 10);

//         assert!(receipt.is_ok());
//         assert_eq!(receipt.unwrap().coins_transferred, 15);
//     }

//     #[test]
//     fn accepts_current_government_log() {
//         let old_log = setup_gov_log_multi(1, 10, 0);
//         let new_log = setup_gov_log_multi(2, 15, 100);
//         let gov = setup_government_node(new_log.clone(), Some(old_log));

//         let mut rng = OsRng;
//         let user_key = SigningKey::generate(&mut rng);

//         let obligation = build_tax_obligation(&user_key, new_log, 25, 1, [3; 32], None);
//         let receipt = sign_tax_receipt(&gov, &obligation, None, 110);

//         assert!(receipt.is_ok());
//         assert_eq!(receipt.unwrap().coins_transferred, 25);
//     }

//     #[test]
//     fn rejects_outdated_government_log() {
//         let ancient_log = setup_gov_log_multi(1, 10, 0);
//         let old_log = setup_gov_log_multi(2, 12, 50);
//         let new_log = setup_gov_log_multi(3, 15, 100);
//         let gov = setup_government_node(new_log, Some(old_log));

//         let mut rng = OsRng;
//         let user_key = SigningKey::generate(&mut rng);

//         let obligation = build_tax_obligation(&user_key, ancient_log, 15, 1, [4; 32], None);
//         let res = sign_tax_receipt(&gov, &obligation, None, 10);

//         assert!(res.is_err());
//         assert!(
//             res.unwrap_err()
//                 .to_string()
//                 .contains("not current or previous")
//         );
//     }

//     #[test]
//     fn rejects_completely_unknown_government_log() {
//         let current_log = setup_gov_log_multi(1, 10, 0);
//         let gov = setup_government_node(current_log, None);

//         let unknown_log = setup_gov_log_multi(99, 50, 999);

//         let mut rng = OsRng;
//         let user_key = SigningKey::generate(&mut rng);

//         let obligation = build_tax_obligation(&user_key, unknown_log, 100, 1, [5; 32], None);
//         let res = sign_tax_receipt(&gov, &obligation, None, 10);

//         assert!(res.is_err());
//         assert!(
//             res.unwrap_err()
//                 .to_string()
//                 .contains("not current or previous")
//         );
//     }

//     #[test]
//     fn fails_on_invalid_user_signature() {
//         let gov_log = setup_gov_log_multi(1, 10, 0);
//         let gov = setup_government_node(gov_log.clone(), None);

//         let mut rng = OsRng;
//         let user_key = SigningKey::generate(&mut rng);

//         let mut obligation = build_tax_obligation(&user_key, gov_log, 20, 1, [6; 32], None);
//         obligation.user_signature = [9; 64];

//         let res = sign_tax_receipt(&gov, &obligation, None, 10);
//         assert!(res.is_err());
//         assert!(
//             res.unwrap_err()
//                 .to_string()
//                 .contains("Invalid user signature")
//         );
//     }

//     #[test]
//     fn fails_on_tampered_obligation_hash() {
//         let gov_log = setup_gov_log_multi(1, 10, 0);
//         let gov = setup_government_node(gov_log.clone(), None);

//         let mut rng = OsRng;
//         let user_key = SigningKey::generate(&mut rng);

//         let mut obligation = build_tax_obligation(&user_key, gov_log, 20, 1, [7; 32], None);
//         obligation.obligation_hash = [99; 32];

//         let res = sign_tax_receipt(&gov, &obligation, None, 10);
//         assert!(res.is_err());
//         assert!(
//             res.unwrap_err()
//                 .to_string()
//                 .contains("Invalid obligation hash")
//         );
//     }

//     #[test]
//     fn fails_on_tampered_coin_amount() {
//         let gov_log = setup_gov_log_multi(1, 10, 0);
//         let gov = setup_government_node(gov_log.clone(), None);

//         let mut rng = OsRng;
//         let user_key = SigningKey::generate(&mut rng);

//         let mut obligation = build_tax_obligation(&user_key, gov_log, 20, 1, [8; 32], None);
//         obligation.coin_amount = 1000;

//         let res = sign_tax_receipt(&gov, &obligation, None, 10);
//         assert!(res.is_err());
//         assert!(
//             res.unwrap_err()
//                 .to_string()
//                 .contains("Invalid obligation hash")
//         );
//     }

//     #[test]
//     fn fails_when_user_signs_with_wrong_key() {
//         let gov_log = setup_gov_log_multi(1, 10, 0);
//         let gov = setup_government_node(gov_log.clone(), None);

//         let mut rng = OsRng;
//         let user_key = SigningKey::generate(&mut rng);
//         let wrong_key = SigningKey::generate(&mut rng);

//         let mut obligation = TaxObligation {
//             event_hash: [9; 32],
//             user_id: VerifyingKey::from(&user_key),
//             coin_amount: 20,
//             government_log: gov_log,
//             sequence_number: 1,
//             user_signature: [0; 64],
//             obligation_hash: [0; 32],
//             previous_receipt_hash: None,
//         };

//         let mut unsigned = obligation.clone();
//         unsigned.user_signature = [0; 64];
//         unsigned.obligation_hash = [0; 32];

//         let hash: HASH = *blake3::hash(&serialize(&unsigned).unwrap()).as_bytes();
//         obligation.obligation_hash = hash;
//         obligation.user_signature = wrong_key.sign(&hash).to_bytes(); 

//         let res = sign_tax_receipt(&gov, &obligation, None, 10);
//         assert!(res.is_err());
//         assert!(
//             res.unwrap_err()
//                 .to_string()
//                 .contains("Invalid user signature")
//         );
//     }

//     #[test]
//     fn receipt_chain_supported() {
//         let gov_log = setup_gov_log_multi(1, 10, 0);
//         let gov = setup_government_node(gov_log.clone(), None);

//         let mut rng = OsRng;
//         let user_key = SigningKey::generate(&mut rng);

//         let first = build_tax_obligation(&user_key, gov_log.clone(), 10, 1, [10; 32], None);
//         let first_receipt = sign_tax_receipt(&gov, &first, None, 10).unwrap();

//         let mut unsigned_first = first_receipt.clone();
//         unsigned_first.government_signature = [0; 64];
//         let prev_hash: HASH = *blake3::hash(&serialize(&unsigned_first).unwrap()).as_bytes();

//         let second =
//             build_tax_obligation(&user_key, gov_log.clone(), 15, 2, [11; 32], Some(prev_hash));
//         let second_receipt = sign_tax_receipt(&gov, &second, Some(prev_hash), 20).unwrap();

//         assert_eq!(second_receipt.previous_receipt_hash, Some(prev_hash));
//         assert_eq!(second_receipt.coins_transferred, 15);
//         assert_eq!(second_receipt.payment_slot, 20);
//     }

//     #[test]
//     fn receipt_chain_with_multiple_payments() {
//         let gov_log = setup_gov_log_multi(1, 10, 0);
//         let gov = setup_government_node(gov_log.clone(), None);

//         let mut rng = OsRng;
//         let user_key = SigningKey::generate(&mut rng);

//         let first = build_tax_obligation(&user_key, gov_log.clone(), 10, 1, [12; 32], None);
//         let first_receipt = sign_tax_receipt(&gov, &first, None, 10).unwrap();

//         let mut unsigned_first = first_receipt.clone();
//         unsigned_first.government_signature = [0; 64];
//         let first_hash: HASH = *blake3::hash(&serialize(&unsigned_first).unwrap()).as_bytes();

//         let second = build_tax_obligation(
//             &user_key,
//             gov_log.clone(),
//             15,
//             2,
//             [13; 32],
//             Some(first_hash),
//         );
//         let second_receipt = sign_tax_receipt(&gov, &second, Some(first_hash), 20).unwrap();

//         let mut unsigned_second = second_receipt.clone();
//         unsigned_second.government_signature = [0; 64];
//         let second_hash: HASH = *blake3::hash(&serialize(&unsigned_second).unwrap()).as_bytes();

//         let third = build_tax_obligation(
//             &user_key,
//             gov_log.clone(),
//             20,
//             3,
//             [14; 32],
//             Some(second_hash),
//         );
//         let third_receipt = sign_tax_receipt(&gov, &third, Some(second_hash), 30).unwrap();

//         assert_eq!(first_receipt.previous_receipt_hash, None);
//         assert_eq!(second_receipt.previous_receipt_hash, Some(first_hash));
//         assert_eq!(third_receipt.previous_receipt_hash, Some(second_hash));

//         assert_eq!(first_receipt.coins_transferred, 10);
//         assert_eq!(second_receipt.coins_transferred, 15);
//         assert_eq!(third_receipt.coins_transferred, 20);
//     }

//     #[test]
//     fn multiple_users_can_submit_obligations() {
//         let gov_log = setup_gov_log_multi(1, 10, 0);
//         let gov = setup_government_node(gov_log.clone(), None);

//         let mut rng = OsRng;
//         let user1_key = SigningKey::generate(&mut rng);
//         let user2_key = SigningKey::generate(&mut rng);
//         let user3_key = SigningKey::generate(&mut rng);

//         let obligation1 = build_tax_obligation(&user1_key, gov_log.clone(), 10, 1, [15; 32], None);
//         let obligation2 = build_tax_obligation(&user2_key, gov_log.clone(), 20, 1, [16; 32], None);
//         let obligation3 = build_tax_obligation(&user3_key, gov_log.clone(), 30, 1, [17; 32], None);

//         let receipt1 = sign_tax_receipt(&gov, &obligation1, None, 10).unwrap();
//         let receipt2 = sign_tax_receipt(&gov, &obligation2, None, 11).unwrap();
//         let receipt3 = sign_tax_receipt(&gov, &obligation3, None, 12).unwrap();

//         assert_eq!(receipt1.coins_transferred, 10);
//         assert_eq!(receipt2.coins_transferred, 20);
//         assert_eq!(receipt3.coins_transferred, 30);

//         for (receipt, expected_coins) in [(receipt1, 10), (receipt2, 20), (receipt3, 30)] {
//             let mut unsigned = receipt.clone();
//             unsigned.government_signature = [0; 64];
//             let hash = blake3::hash(&serialize(&unsigned).unwrap());
//             let sig = Signature::from_bytes(&receipt.government_signature);
//             gov.signing_key
//                 .verifying_key()
//                 .verify(hash.as_bytes(), &sig)
//                 .unwrap();
//             assert_eq!(receipt.coins_transferred, expected_coins);
//         }
//     }

//     #[test]
//     fn zero_coin_obligation_accepted() {
//         let gov_log = setup_gov_log_multi(1, 10, 0);
//         let gov = setup_government_node(gov_log.clone(), None);

//         let mut rng = OsRng;
//         let user_key = SigningKey::generate(&mut rng);

//         let obligation = build_tax_obligation(&user_key, gov_log, 0, 1, [18; 32], None);
//         let receipt = sign_tax_receipt(&gov, &obligation, None, 42).unwrap();

//         assert_eq!(receipt.coins_transferred, 0);
//     }

//     #[test]
//     fn government_log_transition_scenario() {
//         let old_log = setup_gov_log_multi(1, 10, 0);
//         let new_log = setup_gov_log_multi(2, 15, 100);

//         let mut rng = OsRng;
//         let user_key = SigningKey::generate(&mut rng);

//         let mut gov = setup_government_node(old_log.clone(), None);

//         let obligation1 = build_tax_obligation(&user_key, old_log.clone(), 20, 1, [19; 32], None);
//         let receipt1 = sign_tax_receipt(&gov, &obligation1, None, 50);
//         assert!(receipt1.is_ok());

//         gov.previous_log = gov.latest_log.clone();
//         gov.latest_log = Some(new_log.clone());

//         let obligation2 = build_tax_obligation(&user_key, old_log.clone(), 25, 2, [20; 32], None);
//         let receipt2 = sign_tax_receipt(&gov, &obligation2, None, 120);
//         assert!(receipt2.is_ok());

//         let obligation3 = build_tax_obligation(&user_key, new_log.clone(), 30, 3, [21; 32], None);
//         let receipt3 = sign_tax_receipt(&gov, &obligation3, None, 150);
//         assert!(receipt3.is_ok());

//         assert_eq!(receipt1.unwrap().coins_transferred, 20);
//         assert_eq!(receipt2.unwrap().coins_transferred, 25);
//         assert_eq!(receipt3.unwrap().coins_transferred, 30);
//     }
// }

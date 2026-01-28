use crate::types::{Actions, Event, GovernmentRuleLog, HASH, P2PTransferReceipt, Plot, RealmId, ResourceDelta, Resources, UserState};
use crate::utils::serialization::serialize;
use crate::crypto::VDFProof;
use anyhow::{Result, anyhow, bail};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey as PublicKey };
use std::collections::BTreeMap;
#[derive(Debug)]
pub struct UserNode {
    pub realm_id: RealmId,
    pub user_state: UserState,
    pub event_log: Vec<Event>,
    pub signing_key: SigningKey,
}

impl UserNode {
    pub fn new(
        realm_id: RealmId,
        user_id: ed25519_dalek::VerifyingKey,
        plot: Plot,
        signing_key: SigningKey,
    ) -> Self {
        let user_state = UserState {
            user_id,
            resources: Resources {
                wheat: 0,
                wood: 0,
                coins: 0,
            },
            energy: 100,
            plot,
            last_event_hashes: BTreeMap::new(),
            sequence_number: 0,
            government_log_version: 0,
            government_log_hash: [0; 32],
            prev_state_hash: None,
            signature: [0; 64],
            last_p2p_nonce: 0,
        };

        Self {
            realm_id,
            user_state,
            event_log: vec![],
            signing_key,
        }
    }

    pub fn apply_event(&mut self, mut event: Event, gov_log: &GovernmentRuleLog) -> Result<()> {
        if let Actions::Harvest { crop } = &event.action {
            if event.proof.is_none() {
                let planted_hash = self
                    .user_state
                    .plot
                    .planted_at_event_hash
                    .ok_or_else(|| anyhow!("Nothing planted for harvest"))?;
                
                let mut challenge_material = Vec::new();
                challenge_material.extend_from_slice(&planted_hash);
                challenge_material.extend_from_slice(event.plot_id.as_bytes());
                challenge_material.extend_from_slice(self.user_state.user_id.as_bytes());
                let challenge_hash = blake3::hash(&challenge_material).as_bytes().to_vec();

                let rule_key = format!("harvest:{}", crop.as_str().to_lowercase());
                let rule = gov_log
                    .rules
                    .get(&rule_key)
                    .ok_or_else(|| anyhow!("Harvest rule missing"))?;

                event.proof = Some(VDFProof::compute(&challenge_hash, rule.maturation_iterations));
            }
        }

        event.signature = [0; 64];
        let serialized_event = serialize(&event)?;
        let signature = self.signing_key.sign(&serialized_event);
        event.signature = signature.to_bytes();

        Self::verify_event(&event, &self.user_state, gov_log, self.realm_id)?;

        self.user_state.signature = [0; 64];
        let prev_hash: HASH = *blake3::hash(&serialize(&self.user_state)?).as_bytes();

        let rule_key = match &event.action {
            Actions::Plant { crop } => format!("plant:{}", crop.as_str().to_lowercase()),
            Actions::Harvest { crop } => format!("harvest:{}", crop.as_str().to_lowercase()),
        };
        let rule = gov_log
            .rules
            .get(&rule_key)
            .ok_or_else(|| anyhow!("Rule missing during apply"))?;

        self.user_state.energy = self
            .user_state
            .energy
            .checked_sub(rule.energy_cost)
            .ok_or_else(|| anyhow!("Energy underflow"))?;

        self.user_state.resources.wheat =
            self.user_state.resources.wheat.saturating_add_signed(event.deltas.wheat);
        self.user_state.resources.wood =
            self.user_state.resources.wood.saturating_add_signed(event.deltas.wood);
        self.user_state.resources.coins =
            self.user_state.resources.coins.saturating_add_signed(event.deltas.coins);

        let mut unsigned_event = event.clone();
        unsigned_event.signature = [0; 64];
        let event_hash: HASH = *blake3::hash(&serialize(&unsigned_event)?).as_bytes();

        match &event.action {
            Actions::Plant { crop } => {
                self.user_state.plot.crop = Some(crop.as_str().to_string());
                self.user_state.plot.planted_at_event_hash = Some(event_hash);
            }
            Actions::Harvest { .. } => {
                self.user_state.plot.crop = None;
                self.user_state.plot.planted_at_event_hash = None;
            }
        }

        self.user_state
            .last_event_hashes
            .insert(event.plot_id.clone(), event_hash);

        self.user_state.prev_state_hash = Some(prev_hash);
        self.user_state.sequence_number += 1;

        self.user_state.signature = [0; 64];
        let final_hash = blake3::hash(&serialize(&self.user_state)?);
        self.user_state.signature = self.signing_key.sign(final_hash.as_bytes()).to_bytes();

        self.event_log.push(event);

        Ok(())
    }

    pub fn verify_event(
        event: &Event,
        prev_state: &UserState,
        gov_log: &GovernmentRuleLog,
        realm_id: RealmId,
    ) -> Result<()> {
        if gov_log.realm_id != realm_id {
            bail!("Government log belongs to a different realm");
        }

        if event.realm_id != realm_id {
            bail!("Event belongs to a different realm");
        }

        if event.gov_log_version != gov_log.version_number {
            bail!("Government log version mismatch");
        }

        if event.gov_log_hash != gov_log.log_hash {
            bail!("Government log hash mismatch");
        }

        if event.timestamp_slot < gov_log.effective_slot {
            bail!("Event predates government log activation");
        }

        if event.plot_id != prev_state.plot.plot_id {
            bail!("Event targets invalid plot");
        }

        if event.sequence_number != prev_state.sequence_number + 1 {
            bail!("Sequence number discontinuity");
        }

        let expected_prev = prev_state.last_event_hashes.get(&event.plot_id);
        match (expected_prev, &event.prev_event_hash) {
            (Some(expected), Some(provided)) if expected == provided => {}
            (None, None) => {}
            _ => bail!("Plot event hash chain broken"),
        }

        let mut unsigned = event.clone();
        unsigned.signature = [0; 64];
        let sig = Signature::from_bytes(&event.signature);
        prev_state
            .user_id
            .verify(&serialize(&unsigned)?, &sig)
            .map_err(|_| anyhow!("Invalid event signature"))?;

        let rule_key = match &event.action {
            Actions::Plant { crop } => format!("plant:{}", crop.as_str().to_lowercase()),
            Actions::Harvest { crop } => format!("harvest:{}", crop.as_str().to_lowercase()),
        };

        let rule = gov_log
            .rules
            .get(&rule_key)
            .ok_or_else(|| anyhow!("Action {} not allowed", rule_key))?;

        if prev_state.energy < rule.energy_cost {
            bail!("Insufficient energy");
        }

        if event.deltas != rule.resource_reward {
            bail!("Resource delta mismatch");
        }

        match &event.action {
            Actions::Plant { .. } => {
                if prev_state.plot.crop.is_some() {
                    bail!("Plot already occupied");
                }
                if event.proof.is_some() {
                    bail!("Plant event must not include VDF proof");
                }
            }
            Actions::Harvest { crop } => {
                let planted_crop = prev_state
                    .plot
                    .crop
                    .as_ref()
                    .ok_or_else(|| anyhow!("Nothing planted"))?;

                if planted_crop != crop.as_str() {
                    bail!("Crop mismatch");
                }

                let planted_hash = prev_state
                    .plot
                    .planted_at_event_hash
                    .ok_or_else(|| anyhow!("Missing planting anchor"))?;

                let proof = event
                    .proof
                    .as_ref()
                    .ok_or_else(|| anyhow!("Harvest requires VDF proof"))?;

                let mut challenge_material = Vec::new();
                challenge_material.extend_from_slice(&planted_hash);
                challenge_material.extend_from_slice(event.plot_id.as_bytes());
                challenge_material.extend_from_slice(prev_state.user_id.as_bytes());
                let expected_challenge = blake3::hash(&challenge_material).as_bytes().to_vec();

                if proof.challenge != expected_challenge {
                    bail!("Invalid VDF challenge");
                }

                if proof.difficulty != rule.maturation_iterations {
                    bail!("Incorrect VDF difficulty");
                }

                if !proof.verify() {
                    bail!("Invalid VDF proof");
                }
            }
        }

        Ok(())
    }

    pub fn snapshot_recycle(&mut self) {
        self.event_log.clear();
    }
pub fn create_p2p_transfer(
    &self,
    receiver_id: PublicKey,
    receiver_prev_state_hash: HASH,
    receiver_balance: Resources,
    delta: ResourceDelta,
    nonce: u64,
) -> Result<P2PTransferReceipt> {
    if nonce != self.user_state.last_p2p_nonce + 1 {
        bail!("Nonce discontinuity");
    } 

    let sender_balance_after = self.user_state.resources
        .checked_sub(delta)
        .ok_or_else(|| anyhow!("Insufficient balance for transfer"))?;

    let receiver_balance_after = receiver_balance.saturating_add(delta);

    let mut receipt = P2PTransferReceipt {
        realm_id: self.realm_id,
        tx_hash: [0; 32],
        sender: self.user_state.user_id,
        receiver: receiver_id,
        delta,
        sender_prev_state_hash: self.user_state.prev_state_hash
            .ok_or_else(|| anyhow!("Missing sender state hash"))?,
        receiver_prev_state_hash,
        sender_balance_before: self.user_state.resources,
        sender_balance_after,
        receiver_balance_before: receiver_balance,
        receiver_balance_after,
        nonce,
        sender_signature: [0; 64],
        receiver_signature: [0; 64],
    };

    let serialized = serialize(&receipt)?;
    let hash = blake3::hash(&serialized);
    receipt.tx_hash = *hash.as_bytes();

    receipt.sender_signature = self.signing_key.sign(hash.as_bytes()).to_bytes();

    Ok(receipt)
}
pub fn accept_p2p_transfer(
    &self,
    mut receipt: P2PTransferReceipt,
) -> Result<P2PTransferReceipt> {
    if receipt.realm_id != self.realm_id {
        bail!("Receipt from wrong realm");
    }

    if receipt.receiver != self.user_state.user_id {
        bail!("Not the intended receiver");
    }

    if receipt.nonce != self.user_state.last_p2p_nonce + 1 {
        bail!("Nonce discontinuity");
    }

    let mut temp_receipt = receipt.clone();
    temp_receipt.sender_signature = [0; 64];
    temp_receipt.receiver_signature = [0; 64];
    temp_receipt.tx_hash = [0; 32];

    let serialized = serialize(&temp_receipt)?;
    let expected_hash = blake3::hash(&serialized);

    if expected_hash.as_bytes() != &receipt.tx_hash {
        bail!("Receipt hash mismatch");
    }

    let sender_sig = Signature::from_bytes(&receipt.sender_signature);
    receipt.sender
        .verify(receipt.tx_hash.as_slice(), &sender_sig)
        .map_err(|_| anyhow!("Invalid sender signature"))?;

    let receiver_prev = self.user_state.prev_state_hash
        .ok_or_else(|| anyhow!("Missing receiver prev state hash"))?;

    if receipt.receiver_prev_state_hash != receiver_prev {
        bail!("Receiver state mismatch");
    }

    if receipt.receiver_balance_before != self.user_state.resources {
        bail!("Receiver balance mismatch");
    }

    let expected_sender_after = receipt.sender_balance_before
        .checked_sub(receipt.delta)
        .ok_or_else(|| anyhow!("Invalid sender balance subtraction"))?;

    if receipt.sender_balance_after != expected_sender_after {
        bail!("Sender balance arithmetic invalid");
    }

    let expected_receiver_after =
        receipt.receiver_balance_before.saturating_add(receipt.delta);

    if receipt.receiver_balance_after != expected_receiver_after {
        bail!("Receiver balance arithmetic invalid");
    }

    receipt.receiver_signature =
        self.signing_key.sign(&receipt.tx_hash).to_bytes();

    Ok(receipt)
}
pub fn apply_p2p_receipt(&mut self, receipt: &P2PTransferReceipt) -> Result<()> {
    if receipt.realm_id != self.realm_id {
        bail!("Wrong realm");
    }

    let is_sender = receipt.sender == self.user_state.user_id;
    let is_receiver = receipt.receiver == self.user_state.user_id;

    if !is_sender && !is_receiver {
        bail!("Receipt not applicable to this user");
    }

    let mut temp_receipt = receipt.clone();
    temp_receipt.sender_signature = [0; 64];
    temp_receipt.receiver_signature = [0; 64];
    temp_receipt.tx_hash = [0; 32];

    let expected_hash = blake3::hash(&serialize(&temp_receipt)?);

    if expected_hash.as_bytes() != &receipt.tx_hash {
        bail!("Receipt hash verification failed");
    }

    let sender_sig = Signature::from_bytes(&receipt.sender_signature);
    receipt.sender
        .verify(receipt.tx_hash.as_slice(), &sender_sig)
        .map_err(|_| anyhow!("Invalid sender signature"))?;

    let receiver_sig = Signature::from_bytes(&receipt.receiver_signature);
    receipt.receiver
        .verify(receipt.tx_hash.as_slice(), &receiver_sig)
        .map_err(|_| anyhow!("Invalid receiver signature"))?;

    let current_prev_hash = self.user_state.prev_state_hash
        .ok_or_else(|| anyhow!("Missing current prev_state_hash"))?;

    if is_sender && receipt.sender_prev_state_hash != current_prev_hash {
        bail!("Sender state hash mismatch");
    }

    if is_receiver && receipt.receiver_prev_state_hash != current_prev_hash {
        bail!("Receiver state hash mismatch");
    }

    if is_sender && receipt.sender_balance_before != self.user_state.resources {
        bail!("Sender current balance mismatch");
    }

    if is_receiver && receipt.receiver_balance_before != self.user_state.resources {
        bail!("Receiver current balance mismatch");
    }

    let prev_hash = current_prev_hash;

    if is_sender {
        self.user_state.resources = receipt.sender_balance_after;
    } else {
        self.user_state.resources = receipt.receiver_balance_after;
    }

    self.user_state.prev_state_hash = Some(prev_hash);
    self.user_state.sequence_number += 1;
    self.user_state.last_p2p_nonce = receipt.nonce;

    self.user_state.signature = [0; 64];
    let final_hash = blake3::hash(&serialize(&self.user_state)?);
    self.user_state.signature =
        self.signing_key.sign(final_hash.as_bytes()).to_bytes();

    Ok(())
}


}



// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::types::{ActionRule, Crops, Event, GovernmentRuleLog, Plot, ResourceDelta};
//     use ed25519_dalek::{SigningKey, VerifyingKey};
//     use rand::rngs::OsRng;
//     use std::collections::BTreeMap;

//     fn setup_user_node_multi() -> (UserNode, SigningKey) {
//         let mut csprng = OsRng;
//         let signing_key = SigningKey::generate(&mut csprng);
//         let verifying_key = VerifyingKey::from(&signing_key);
//         let plot = Plot {
//             plot_id: "plot_1".to_string(),
//             crop: None,
//             planted_at_event_hash: None,
//         };
//         (UserNode::new([0;32],verifying_key, plot, signing_key.clone()), signing_key)
//     }

//     fn setup_gov_log_multi() -> GovernmentRuleLog {
//         let mut rules = BTreeMap::new();
//         for crop in ["wheat", "corn"] {
//             rules.insert(
//                 format!("plant:{}", crop),
//                 ActionRule {
//                     energy_cost: 10,
//                     resource_reward: ResourceDelta { wheat: 0, wood: 0, coins: 0 },
//                     maturation_iterations: 1_000,
//                 },
//             );
//             rules.insert(
//                 format!("harvest:{}", crop),
//                 ActionRule {
//                     energy_cost: 5,
//                     resource_reward: ResourceDelta { wheat: 50, wood: 0, coins: 20 },
//                     maturation_iterations: 1_000,
//                 },
//             );
//         }

//         GovernmentRuleLog {
//             version_number: 1,
//             effective_slot: 0,
//             rules,
//             tasks: vec![],
//             tax_rate_percent: 10,
//             tax_deadline: 100,
//             log_hash: [0; 32],
//             government_signature: [0; 64],
//             previous_log_hash: None,
//             realm_id: [0;32],
//         }
//     }

//     #[test]
//     fn test_auto_vdf_harvest() {
//         let (mut node, _signing_key) = setup_user_node_multi();
//         let gov_log = setup_gov_log_multi();

//         let plant_evt = Event {
//             event_id: "evt_plant".to_string(),
//             user_id: node.user_state.user_id,
//             plot_id: "plot_1".to_string(),
//             action: Actions::Plant { crop: Crops::Wheat },
//             task_id: None,
//             deltas: ResourceDelta { wheat: 0, wood: 0, coins: 0 },
//             gov_log_version: gov_log.version_number,
//             gov_log_hash: gov_log.log_hash,
//             prev_event_hash: None,
//             signature: [0; 64],
//             timestamp_slot: 1,
//             proof: None,
//             sequence_number: 1,
//             realm_id: node.realm_id,
//         };
        
//         node.apply_event(plant_evt, &gov_log).unwrap();

//         let harvest_evt = Event {
//             event_id: "evt_harvest".to_string(),
//             user_id: node.user_state.user_id,
//             plot_id: "plot_1".to_string(),
//             action: Actions::Harvest { crop: Crops::Wheat },
//             task_id: None,
//             deltas: gov_log.rules.get("harvest:wheat").unwrap().resource_reward.clone(),
//             gov_log_version: gov_log.version_number,
//             gov_log_hash: gov_log.log_hash,
//             prev_event_hash: Some(*node.user_state.last_event_hashes.get("plot_1").unwrap()),
//             signature: [0; 64],
//             timestamp_slot: 2,
//             proof: None, 
//             sequence_number: 2,
//             realm_id: node.realm_id,
//         };

//         let res = node.apply_event(harvest_evt, &gov_log);
//         assert!(res.is_ok());
//         assert_eq!(node.user_state.plot.crop, None);
//         assert!(node.user_state.resources.wheat > 0);
//     }

//     #[test]
//     fn test_snapshot_recycle() {
//         let (mut node, _signing_key) = setup_user_node_multi();
//         let gov_log = setup_gov_log_multi();

//         let plant_evt = Event {
//             event_id: "evt_plant_snapshot".to_string(),
//             user_id: node.user_state.user_id,
//             plot_id: "plot_1".to_string(),
//             action: Actions::Plant { crop: Crops::Wheat },
//             task_id: None,
//             deltas: ResourceDelta { wheat: 0, wood: 0, coins: 0 },
//             gov_log_version: gov_log.version_number,
//             gov_log_hash: gov_log.log_hash,
//             prev_event_hash: None,
//             signature: [0; 64],
//             timestamp_slot: 1,
//             proof: None,
//             sequence_number: 1,
//             realm_id: node.realm_id,
//         };

//         node.apply_event(plant_evt, &gov_log).unwrap();

//         node.snapshot_recycle();
//         assert_eq!(node.event_log.len(), 0);
//         assert_eq!(node.user_state.plot.crop, Some("wheat".to_string()));
//     }

//     #[test]
//     #[ignore]
//     fn test_vdf_timing_benchmark() {
//         let difficulties = vec![1_000, 10_000, 100_000, 1_000_000, 10_000_000];
        
//         println!("\n=== VDF Timing Benchmark ===");
//         for diff in difficulties {
//             let challenge = b"timing-test";
//             let start = std::time::Instant::now();
//             let proof = VDFProof::compute(challenge, diff);
//             let duration = start.elapsed();
            
//             println!("Difficulty {:>10}: {:>8.2?}", diff, duration);
//             assert!(proof.verify());
//         }
//     }

// #[cfg(test)]
// mod p2p_tests {
//     use super::*;
//     use crate::types::{ActionRule, Crops, GovernmentRuleLog, Plot, ResourceDelta, Resources};
//     use ed25519_dalek::{SigningKey, VerifyingKey};
//     use rand::rngs::OsRng;
//     use std::collections::BTreeMap;

//     /// Setup two users in the same realm
//     fn setup_two_users_same_realm() -> (UserNode, UserNode, RealmId) {
//         let mut csprng = OsRng;
//         let realm_id = [1u8; 32];

//         // User 1 (Alice)
//         let signing_key_1 = SigningKey::generate(&mut csprng);
//         let verifying_key_1 = VerifyingKey::from(&signing_key_1);
//         let plot_1 = Plot {
//             plot_id: "plot_alice".to_string(),
//             crop: None,
//             planted_at_event_hash: None,
//         };
//         let mut alice = UserNode::new(realm_id, verifying_key_1, plot_1, signing_key_1);

//         // Give Alice some initial resources
//         alice.user_state.resources = Resources {
//             wheat: 100,
//             wood: 50,
//             coins: 200,
//         };
//         // Initialize prev_state_hash
//         alice.user_state.signature = [0; 64];
//         let initial_hash = *blake3::hash(&serialize(&alice.user_state).unwrap()).as_bytes();
//         alice.user_state.prev_state_hash = Some(initial_hash);

//         // User 2 (Bob)
//         let signing_key_2 = SigningKey::generate(&mut csprng);
//         let verifying_key_2 = VerifyingKey::from(&signing_key_2);
//         let plot_2 = Plot {
//             plot_id: "plot_bob".to_string(),
//             crop: None,
//             planted_at_event_hash: None,
//         };
//         let mut bob = UserNode::new(realm_id, verifying_key_2, plot_2, signing_key_2);

//         // Give Bob some initial resources
//         bob.user_state.resources = Resources {
//             wheat: 50,
//             wood: 100,
//             coins: 150,
//         };
//         // Initialize prev_state_hash
//         bob.user_state.signature = [0; 64];
//         let initial_hash = *blake3::hash(&serialize(&bob.user_state).unwrap()).as_bytes();
//         bob.user_state.prev_state_hash = Some(initial_hash);

//         (alice, bob, realm_id)
//     }

//     /// Setup two users in different realms
//     fn setup_two_users_different_realms() -> (UserNode, UserNode) {
//         let mut csprng = OsRng;
//         let realm_id_1 = [1u8; 32];
//         let realm_id_2 = [2u8; 32];

//         // User 1 in realm 1
//         let signing_key_1 = SigningKey::generate(&mut csprng);
//         let verifying_key_1 = VerifyingKey::from(&signing_key_1);
//         let plot_1 = Plot {
//             plot_id: "plot_1".to_string(),
//             crop: None,
//             planted_at_event_hash: None,
//         };
//         let mut user1 = UserNode::new(realm_id_1, verifying_key_1, plot_1, signing_key_1);
//         user1.user_state.resources = Resources {
//             wheat: 100,
//             wood: 50,
//             coins: 200,
//         };
//         user1.user_state.signature = [0; 64];
//         let initial_hash = *blake3::hash(&serialize(&user1.user_state).unwrap()).as_bytes();
//         user1.user_state.prev_state_hash = Some(initial_hash);

//         // User 2 in realm 2
//         let signing_key_2 = SigningKey::generate(&mut csprng);
//         let verifying_key_2 = VerifyingKey::from(&signing_key_2);
//         let plot_2 = Plot {
//             plot_id: "plot_2".to_string(),
//             crop: None,
//             planted_at_event_hash: None,
//         };
//         let mut user2 = UserNode::new(realm_id_2, verifying_key_2, plot_2, signing_key_2);
//         user2.user_state.resources = Resources {
//             wheat: 50,
//             wood: 100,
//             coins: 150,
//         };
//         user2.user_state.signature = [0; 64];
//         let initial_hash = *blake3::hash(&serialize(&user2.user_state).unwrap()).as_bytes();
//         user2.user_state.prev_state_hash = Some(initial_hash);

//         (user1, user2)
//     }

//     #[test]
//     fn test_p2p_happy_path() {
//         let (mut alice, mut bob, _realm_id) = setup_two_users_same_realm();

//         println!("=== Initial State ===");
//         println!("Alice: {:?}", alice.user_state.resources);
//         println!("Bob: {:?}", bob.user_state.resources);

//         // Alice sends 30 wheat to Bob
//         let delta = ResourceDelta {
//             wheat: 30,
//             wood: 0,
//             coins: 0,
//         };

//         let receipt = alice
//             .create_p2p_transfer(
//                 bob.user_state.user_id,
//                 bob.user_state.prev_state_hash.unwrap(),
//                 bob.user_state.resources,
//                 delta,
//                 1, // nonce
//             )
//             .expect("Failed to create transfer");

//         println!("\n=== Transfer Created ===");
//         println!("Delta: {:?}", delta);
//         println!("Nonce: {}", receipt.nonce);

//         // Bob accepts the transfer
//         let signed_receipt = bob
//             .accept_p2p_transfer(receipt)
//             .expect("Failed to accept transfer");

//         println!("\n=== Transfer Accepted ===");

//         // Both apply the receipt
//         alice
//             .apply_p2p_receipt(&signed_receipt)
//             .expect("Alice failed to apply receipt");
//         bob.apply_p2p_receipt(&signed_receipt)
//             .expect("Bob failed to apply receipt");

//         println!("\n=== Final State ===");
//         println!("Alice: {:?}", alice.user_state.resources);
//         println!("Bob: {:?}", bob.user_state.resources);

//         // Verify final balances
//         assert_eq!(alice.user_state.resources.wheat, 70); // 100 - 30
//         assert_eq!(bob.user_state.resources.wheat, 80); // 50 + 30
//         assert_eq!(alice.user_state.last_p2p_nonce, 1);
//         assert_eq!(bob.user_state.last_p2p_nonce, 1);
//     }

//     #[test]
//    #[test]
// fn test_p2p_cross_realm_rejected() {
//     let (mut alice, mut bob) = setup_two_users_different_realms();

//     println!("=== Testing Cross-Realm Transfer ===");
//     println!("Alice realm: {:?}", alice.realm_id);
//     println!("Bob realm: {:?}", bob.realm_id);

//     let delta = ResourceDelta {
//         wheat: 10,
//         wood: 0,
//         coins: 0,
//     };

//     // Alice creates the transfer (this will succeed)
//     let receipt = alice.create_p2p_transfer(
//         bob.user_state.user_id,
//         bob.user_state.prev_state_hash.unwrap(),
//         bob.user_state.resources,
//         delta,
//         1,
//     ).expect("Alice creates receipt");

//     // Bob tries to accept - THIS should fail due to realm mismatch
//     let result = bob.accept_p2p_transfer(receipt);

//     assert!(result.is_err());
//     let err = result.unwrap_err().to_string();
//     assert!(err.contains("realm") || err.contains("wrong realm"));
//     println!("✓ Cross-realm transfer correctly rejected: {}", err);
// }

//     #[test]
//     fn test_p2p_wrong_nonce_rejected() {
//         let (mut alice, mut bob, _) = setup_two_users_same_realm();

//         println!("=== Testing Wrong Nonce ===");
//         println!("Alice last nonce: {}", alice.user_state.last_p2p_nonce);

//         let delta = ResourceDelta {
//             wheat: 10,
//             wood: 0,
//             coins: 0,
//         };

//         // Try with wrong nonce (should be 1, trying 5)
//         let result = alice.create_p2p_transfer(
//             bob.user_state.user_id,
//             bob.user_state.prev_state_hash.unwrap(),
//             bob.user_state.resources,
//             delta,
//             5, // Wrong nonce!
//         );

//         assert!(result.is_err());
//         let err = result.unwrap_err().to_string();
//         assert!(err.contains("Nonce"));
//         println!("✓ Wrong nonce correctly rejected: {}", err);
//     }

//     #[test]
//     fn test_p2p_nonce_sequence() {
//         let (mut alice, mut bob, _) = setup_two_users_same_realm();

//         println!("=== Testing Nonce Sequence ===");

//         // First transfer with nonce 1
//         let delta1 = ResourceDelta {
//             wheat: 10,
//             wood: 0,
//             coins: 0,
//         };
//         let receipt1 = alice
//             .create_p2p_transfer(
//                 bob.user_state.user_id,
//                 bob.user_state.prev_state_hash.unwrap(),
//                 bob.user_state.resources,
//                 delta1,
//                 1,
//             )
//             .unwrap();

//         let signed_receipt1 = bob.accept_p2p_transfer(receipt1).unwrap();
//         alice.apply_p2p_receipt(&signed_receipt1).unwrap();
//         bob.apply_p2p_receipt(&signed_receipt1).unwrap();

//         println!("First transfer complete. Alice nonce: {}", alice.user_state.last_p2p_nonce);

//         // Second transfer with nonce 2
//         let delta2 = ResourceDelta {
//             wheat: 5,
//             wood: 0,
//             coins: 0,
//         };
//         let receipt2 = alice
//             .create_p2p_transfer(
//                 bob.user_state.user_id,
//                 bob.user_state.prev_state_hash.unwrap(),
//                 bob.user_state.resources,
//                 delta2,
//                 2,
//             )
//             .unwrap();

//         let signed_receipt2 = bob.accept_p2p_transfer(receipt2).unwrap();
//         alice.apply_p2p_receipt(&signed_receipt2).unwrap();
//         bob.apply_p2p_receipt(&signed_receipt2).unwrap();

//         println!("Second transfer complete. Alice nonce: {}", alice.user_state.last_p2p_nonce);

//         assert_eq!(alice.user_state.last_p2p_nonce, 2);
//         assert_eq!(bob.user_state.last_p2p_nonce, 2);
//         assert_eq!(alice.user_state.resources.wheat, 85); // 100 - 10 - 5
//         assert_eq!(bob.user_state.resources.wheat, 65); // 50 + 10 + 5
//     }

//     #[test]
//     fn test_p2p_insufficient_balance() {
//         let (mut alice, bob, _) = setup_two_users_same_realm();

//         println!("=== Testing Insufficient Balance ===");
//         println!("Alice wheat: {}", alice.user_state.resources.wheat);

//         // Try to send more wheat than Alice has
//         let delta = ResourceDelta {
//             wheat: 150, // Alice only has 100
//             wood: 0,
//             coins: 0,
//         };

//         let result = alice.create_p2p_transfer(
//             bob.user_state.user_id,
//             bob.user_state.prev_state_hash.unwrap(),
//             bob.user_state.resources,
//             delta,
//             1,
//         );

//         assert!(result.is_err());
//         let err = result.unwrap_err().to_string();
//         assert!(err.contains("Insufficient balance"));
//         println!("✓ Insufficient balance correctly rejected: {}", err);
//     }

//     #[test]
//     fn test_p2p_fraudulent_balance_tampering() {
//         let (mut alice, mut bob, _) = setup_two_users_same_realm();

//         println!("=== Testing Fraudulent Balance Tampering ===");

//         let delta = ResourceDelta {
//             wheat: 30,
//             wood: 0,
//             coins: 0,
//         };

//         let mut receipt = alice
//             .create_p2p_transfer(
//                 bob.user_state.user_id,
//                 bob.user_state.prev_state_hash.unwrap(),
//                 bob.user_state.resources,
//                 delta,
//                 1,
//             )
//             .unwrap();

//         // FRAUD: Alice tries to tamper with the balance after calculation
//         receipt.sender_balance_after.wheat = 90; // Should be 70, trying to keep extra

//         // Bob should detect this during acceptance
//         let result = bob.accept_p2p_transfer(receipt);

//         assert!(result.is_err());
//         let err = result.unwrap_err().to_string();
//         assert!(err.contains("hash") || err.contains("arithmetic") || err.contains("Sender balance"));
//         println!("✓ Balance tampering correctly detected: {}", err);
//     }

//     #[test]
//     fn test_p2p_fraudulent_signature() {
//         let (mut alice, mut bob, _) = setup_two_users_same_realm();

//         println!("=== Testing Fraudulent Signature ===");

//         let delta = ResourceDelta {
//             wheat: 30,
//             wood: 0,
//             coins: 0,
//         };

//         let mut receipt = alice
//             .create_p2p_transfer(
//                 bob.user_state.user_id,
//                 bob.user_state.prev_state_hash.unwrap(),
//                 bob.user_state.resources,
//                 delta,
//                 1,
//             )
//             .unwrap();

//         // FRAUD: Tamper with the signature
//         receipt.sender_signature[0] ^= 0xFF;

//         // Bob should detect invalid signature
//         let result = bob.accept_p2p_transfer(receipt);

//         assert!(result.is_err());
//         let err = result.unwrap_err().to_string();
//         assert!(err.contains("signature") || err.contains("Signature"));
//         println!("✓ Invalid signature correctly detected: {}", err);
//     }

//     #[test]
//     fn test_p2p_state_hash_mismatch() {
//         let (mut alice, mut bob, _) = setup_two_users_same_realm();

//         println!("=== Testing State Hash Mismatch ===");

//         // Bob's state changes before accepting transfer
//         bob.user_state.resources.coins += 10;
//         bob.user_state.signature = [0; 64];
//         let new_hash = *blake3::hash(&serialize(&bob.user_state).unwrap()).as_bytes();
//         bob.user_state.prev_state_hash = Some(new_hash);

//         let delta = ResourceDelta {
//             wheat: 30,
//             wood: 0,
//             coins: 0,
//         };

//         // Alice creates transfer with Bob's OLD state hash
//         let old_bob_hash = [0u8; 32]; // Wrong hash
//         let receipt = alice
//             .create_p2p_transfer(
//                 bob.user_state.user_id,
//                 old_bob_hash,
//                 Resources { wheat: 50, wood: 100, coins: 150 }, // Old balance
//                 delta,
//                 1,
//             )
//             .unwrap();

//         // Bob should reject due to state mismatch
//         let result = bob.accept_p2p_transfer(receipt);

//         assert!(result.is_err());
//         let err = result.unwrap_err().to_string();
//         assert!(err.contains("state") || err.contains("balance"));
//         println!("✓ State hash mismatch correctly detected: {}", err);
//     }

//     #[test]
//     fn test_p2p_multi_resource_transfer() {
//         let (mut alice, mut bob, _) = setup_two_users_same_realm();

//         println!("=== Testing Multi-Resource Transfer ===");

//         // Transfer multiple resource types
//         let delta = ResourceDelta {
//             wheat: 20,
//             wood: 10,
//             coins: 50,
//         };

//         let receipt = alice
//             .create_p2p_transfer(
//                 bob.user_state.user_id,
//                 bob.user_state.prev_state_hash.unwrap(),
//                 bob.user_state.resources,
//                 delta,
//                 1,
//             )
//             .expect("Failed to create transfer");

//         let signed_receipt = bob
//             .accept_p2p_transfer(receipt)
//             .expect("Failed to accept transfer");

//         alice.apply_p2p_receipt(&signed_receipt).unwrap();
//         bob.apply_p2p_receipt(&signed_receipt).unwrap();

//         // Verify all resources transferred correctly
//         assert_eq!(alice.user_state.resources.wheat, 80); // 100 - 20
//         assert_eq!(alice.user_state.resources.wood, 40); // 50 - 10
//         assert_eq!(alice.user_state.resources.coins, 150); // 200 - 50

//         assert_eq!(bob.user_state.resources.wheat, 70); // 50 + 20
//         assert_eq!(bob.user_state.resources.wood, 110); // 100 + 10
//         assert_eq!(bob.user_state.resources.coins, 200); // 150 + 50

//         println!("✓ Multi-resource transfer successful");
//     }

//     #[test]
//     fn test_p2p_replay_attack_prevention() {
//         let (mut alice, mut bob, _) = setup_two_users_same_realm();

//         println!("=== Testing Replay Attack Prevention ===");

//         let delta = ResourceDelta {
//             wheat: 10,
//             wood: 0,
//             coins: 0,
//         };

//         // First transfer
//         let receipt = alice
//             .create_p2p_transfer(
//                 bob.user_state.user_id,
//                 bob.user_state.prev_state_hash.unwrap(),
//                 bob.user_state.resources,
//                 delta,
//                 1,
//             )
//             .unwrap();

//         let signed_receipt = bob.accept_p2p_transfer(receipt).unwrap();
//         alice.apply_p2p_receipt(&signed_receipt).unwrap();
//         bob.apply_p2p_receipt(&signed_receipt).unwrap();

//         println!("First transfer applied successfully");

//         // Try to replay the same receipt
//         let result = alice.apply_p2p_receipt(&signed_receipt);

//         assert!(result.is_err());
//         let err = result.unwrap_err().to_string();
//         // Should fail due to state hash mismatch or balance mismatch
//         assert!(err.contains("mismatch") || err.contains("hash"));
//         println!("✓ Replay attack correctly prevented: {}", err);
//     }
// }
// }
use crate::types::{Actions, Event, GovernmentRuleLog, HASH, Plot, Resources, UserState};
use crate::utils::serialization::serialize;
use crate::crypto::VDFProof;
use anyhow::{Result, anyhow, bail};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct UserNode {
    pub user_state: UserState,
    pub event_log: Vec<Event>,
    pub signing_key: SigningKey,
}

impl UserNode {
    pub fn new(user_id: ed25519_dalek::VerifyingKey, plot: Plot, signing_key: SigningKey) -> Self {
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
        };

        Self {
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

        Self::verify_event(&event, &self.user_state, gov_log)?;

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
    ) -> Result<()> {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ActionRule, Crops, Event, GovernmentRuleLog, Plot, ResourceDelta};
    use ed25519_dalek::{SigningKey, VerifyingKey};
    use rand::rngs::OsRng;
    use std::collections::BTreeMap;

    fn setup_user_node_multi() -> (UserNode, SigningKey) {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = VerifyingKey::from(&signing_key);
        let plot = Plot {
            plot_id: "plot_1".to_string(),
            crop: None,
            planted_at_event_hash: None,
        };
        (UserNode::new(verifying_key, plot, signing_key.clone()), signing_key)
    }

    fn setup_gov_log_multi() -> GovernmentRuleLog {
        let mut rules = BTreeMap::new();
        for crop in ["wheat", "corn"] {
            rules.insert(
                format!("plant:{}", crop),
                ActionRule {
                    energy_cost: 10,
                    resource_reward: ResourceDelta { wheat: 0, wood: 0, coins: 0 },
                    maturation_iterations: 1_000_000,
                },
            );
            rules.insert(
                format!("harvest:{}", crop),
                ActionRule {
                    energy_cost: 5,
                    resource_reward: ResourceDelta { wheat: 50, wood: 0, coins: 20 },
                    maturation_iterations: 1_000_000,
                },
            );
        }

        GovernmentRuleLog {
            version_number: 1,
            effective_slot: 0,
            rules,
            tasks: vec![],
            tax_rate_percent: 10,
            tax_deadline: 100,
            log_hash: [0; 32],
            government_signature: [0; 64],
        }
    }

    #[test]
    fn test_auto_vdf_harvest() {
        let (mut node, _signing_key) = setup_user_node_multi();
        let gov_log = setup_gov_log_multi();

        let plant_evt = Event {
            event_id: "evt_plant".to_string(),
            user_id: node.user_state.user_id,
            plot_id: "plot_1".to_string(),
            action: Actions::Plant { crop: Crops::Wheat },
            task_id: None,
            deltas: ResourceDelta { wheat: 0, wood: 0, coins: 0 },
            gov_log_version: gov_log.version_number,
            gov_log_hash: gov_log.log_hash,
            prev_event_hash: None,
            signature: [0; 64],
            timestamp_slot: 1,
            proof: None,
            sequence_number: 1,
        };
        
        node.apply_event(plant_evt, &gov_log).unwrap();

        let harvest_evt = Event {
            event_id: "evt_harvest".to_string(),
            user_id: node.user_state.user_id,
            plot_id: "plot_1".to_string(),
            action: Actions::Harvest { crop: Crops::Wheat },
            task_id: None,
            deltas: gov_log.rules.get("harvest:wheat").unwrap().resource_reward.clone(),
            gov_log_version: gov_log.version_number,
            gov_log_hash: gov_log.log_hash,
            prev_event_hash: Some(*node.user_state.last_event_hashes.get("plot_1").unwrap()),
            signature: [0; 64],
            timestamp_slot: 2,
            proof: None, 
            sequence_number: 2,
        };

        let res = node.apply_event(harvest_evt, &gov_log);
        assert!(res.is_ok());
        assert_eq!(node.user_state.plot.crop, None);
        assert!(node.user_state.resources.wheat > 0);
    }

    #[test]
    fn test_snapshot_recycle() {
        let (mut node, _signing_key) = setup_user_node_multi();
        let gov_log = setup_gov_log_multi();

        let plant_evt = Event {
            event_id: "evt_plant_snapshot".to_string(),
            user_id: node.user_state.user_id,
            plot_id: "plot_1".to_string(),
            action: Actions::Plant { crop: Crops::Wheat },
            task_id: None,
            deltas: ResourceDelta { wheat: 0, wood: 0, coins: 0 },
            gov_log_version: gov_log.version_number,
            gov_log_hash: gov_log.log_hash,
            prev_event_hash: None,
            signature: [0; 64],
            timestamp_slot: 1,
            proof: None,
            sequence_number: 1,
        };

        node.apply_event(plant_evt, &gov_log).unwrap();

        node.snapshot_recycle();
        assert_eq!(node.event_log.len(), 0);
        assert_eq!(node.user_state.plot.crop, Some("wheat".to_string()));
    }

    #[test]
    #[ignore]
    fn test_vdf_timing_benchmark() {
        let difficulties = vec![1_000, 10_000, 100_000, 1_000_000, 10_000_000];
        
        println!("\n=== VDF Timing Benchmark ===");
        for diff in difficulties {
            let challenge = b"timing-test";
            let start = std::time::Instant::now();
            let proof = VDFProof::compute(challenge, diff);
            let duration = start.elapsed();
            
            println!("Difficulty {:>10}: {:>8.2?}", diff, duration);
            assert!(proof.verify());
        }
    }
}
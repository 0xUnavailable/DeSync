use crate::types::*;
use crate::crypto::VDFProof;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::utils::persistence::{append_event, PersistentRecord};

#[derive(Serialize, Deserialize)]
pub struct UserStateManager {
    pub active_tasks: HashMap<String, Task>,
    pub resources: Resources,
    pub coins: i64,  // Changed to i64 to support negative balances (debt)
    pub completed_tasks: Vec<CompletedTask>,
    pub tax_payments: Vec<TaxPaymentRecord>,
    #[serde(default)]
    pub paid_log_versions: HashMap<u64, bool>,  // Track which government log versions have been paid
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CompletedTask {
    pub task_name: String,
    pub completed_at: u64,
    pub coins_earned: u64,
    pub vdf_proof: VDFProof,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TaxPaymentRecord {
    pub paid_at: u64,
    pub amount: u64,
    pub gov_log_version: u64,  // Track which government log version this payment is for
    pub obligation_hash: HASH,
    pub receipt_hash: Option<HASH>,
}

impl UserStateManager {
    pub fn new() -> Self {
        Self {
            active_tasks: HashMap::new(),
            resources: Resources {
                land: 100,
                labor: 100,
                capital: 50,
            },
            coins: 0,
            completed_tasks: Vec::new(),
            tax_payments: Vec::new(),
            paid_log_versions: HashMap::new(),
        }
    }
    
    pub fn start_task(&mut self, task_name: String, rule: &ActionRule) -> Result<(), String> {
        if self.active_tasks.contains_key(&task_name) {
            return Err("Task already active".to_string());
        }
        
        // Check resources
        if self.resources.land < rule.required_resources.land {
            return Err(format!(
                "Insufficient land (need {}, have {})",
                rule.required_resources.land,
                self.resources.land
            ));
        }
        if self.resources.labor < rule.required_resources.labor {
            return Err(format!(
                "Insufficient labor (need {}, have {})",
                rule.required_resources.labor,
                self.resources.labor
            ));
        }
        if self.resources.capital < rule.required_resources.capital {
            return Err(format!(
                "Insufficient capital (need {}, have {})",
                rule.required_resources.capital,
                self.resources.capital
            ));
        }
        
        // Lock resources
        self.resources.land -= rule.required_resources.land;
        self.resources.labor -= rule.required_resources.labor;
        self.resources.capital -= rule.required_resources.capital;
        
        println!("[STATE] Resources locked:");
        println!("  Land: -{}, Labor: -{}, Capital: -{}",
            rule.required_resources.land,
            rule.required_resources.labor,
            rule.required_resources.capital
        );
        
        let task = Task {
            task_name: task_name.clone(),
            start_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            duration: 60,
            resources_locked: rule.required_resources.clone(),
        };
        
        self.active_tasks.insert(task_name.clone(), task.clone());
        
        // Log task started event
        let event = crate::utils::persistence::TaskStartedEvent {
            task_name: task.task_name.clone(),
            start_time: task.start_time,
            duration: task.duration,
            resources_locked: task.resources_locked.clone(),
        };
        let _ = append_event(&PersistentRecord::TaskStarted(event));
        
        Ok(())
    }
    
    pub fn harvest_task(&mut self, task_name: &str, multiplier: u64) -> Result<(u64, VDFProof), String> {
        let task = self.active_tasks.remove(task_name)
            .ok_or_else(|| format!("Task '{}' not found in active tasks", task_name))?;
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if current_time < task.start_time + task.duration {
            let remaining = (task.start_time + task.duration) - current_time;
            self.active_tasks.insert(task_name.to_string(), task);
            return Err(format!("Task not complete yet. {} seconds remaining", remaining));
        }
        
        // Compute VDF proof to demonstrate actual work was done
        println!("[VDF] Computing proof of work for task '{}'...", task_name);
        let challenge = format!("{}-{}-{}", task_name, task.start_time, current_time);
        let iterations = 5_000_000;  // Increased to ~10-20 seconds on modern hardware
        
        let start_compute = std::time::Instant::now();
        let vdf_proof = VDFProof::compute(challenge.as_bytes(), iterations);
        let compute_duration = start_compute.elapsed();
        
        println!("[VDF] Proof computed in {:.2}s ({} iterations)",
            compute_duration.as_secs_f64(),
            iterations
        );
        
        // Unlock resources
        self.resources.land += task.resources_locked.land;
        self.resources.labor += task.resources_locked.labor;
        self.resources.capital += task.resources_locked.capital;
        
        println!("[STATE] Resources unlocked:");
        println!("  Land: +{}, Labor: +{}, Capital: +{}",
            task.resources_locked.land,
            task.resources_locked.labor,
            task.resources_locked.capital
        );
        
        // Calculate coins earned
        let coins_earned = multiplier;
        self.coins += coins_earned as i64;
        
        println!("[STATE] Coin balance updated:");
        println!("  Earned: {} coins", coins_earned);
        println!("  New balance: {} coins", self.coins);
        
        // Record completed task
        let completed = CompletedTask {
            task_name: task_name.to_string(),
            completed_at: current_time,
            coins_earned,
            vdf_proof: vdf_proof.clone(),
        };
        self.completed_tasks.push(completed);
        
        // Log task harvested event
        let event = crate::utils::persistence::TaskHarvestedEvent {
            task_name: task_name.to_string(),
            completed_at: current_time,
            coins_earned,
            vdf_iterations: vdf_proof.iterations,
            vdf_compute_time_ms: (compute_duration.as_secs_f64() * 1000.0) as u64,
        };
        let _ = append_event(&PersistentRecord::TaskHarvested(event));
        
        Ok((coins_earned, vdf_proof))
    }
    
    /// Record a tax payment (obligation submitted)
    pub fn record_tax_payment(&mut self, amount: u64, gov_log_version: u64, obligation_hash: HASH) -> Result<(), String> {
        if self.coins < amount as i64 {
            return Err(format!(
                "Insufficient coins for tax payment (need {}, have {})",
                amount,
                self.coins
            ));
        }
        
        // Check if already paid for this version
        if self.paid_log_versions.get(&gov_log_version).copied().unwrap_or(false) {
            return Err(format!(
                "Tax already paid for government log version {}",
                gov_log_version
            ));
        }
        
        // Deduct coins for tax payment
        self.coins -= amount as i64;
        
        println!("[STATE] Tax payment recorded:");
        println!("  Amount: {} coins", amount);
        println!("  Gov log version: {}", gov_log_version);
        println!("  New balance: {} coins", self.coins);
        
        let payment = TaxPaymentRecord {
            paid_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            amount,
            gov_log_version,
            obligation_hash,
            receipt_hash: None,
        };
        
        self.tax_payments.push(payment);
        self.paid_log_versions.insert(gov_log_version, true);
        
        Ok(())
    }
    
    /// Process received tax receipt (coins are credited back)
    pub fn process_tax_receipt(&mut self, obligation_hash: HASH, receipt_hash: HASH, coins_transferred: u64) {
        println!("[DEBUG] Processing tax receipt:");
        println!("  Obligation hash: {}", hex::encode(obligation_hash));
        println!("  Receipt hash: {}", hex::encode(receipt_hash));
        println!("  Coins: {}", coins_transferred);
        println!("  Current payments: {}", self.tax_payments.len());
        
        // Debug: Print all payment hashes
        for (i, payment) in self.tax_payments.iter().enumerate() {
            println!("  Payment {}: hash={}, receipt={}", 
                i, 
                hex::encode(payment.obligation_hash),
                if payment.receipt_hash.is_some() { "present" } else { "missing" }
            );
        }
        
        // Find the corresponding payment record
        if let Some(payment) = self.tax_payments.iter_mut()
            .find(|p| p.obligation_hash == obligation_hash && p.receipt_hash.is_none())
        {
            payment.receipt_hash = Some(receipt_hash);
            
            // Credit back the coins (receipt confirms government received the obligation)
            self.coins += coins_transferred as i64;
            
            println!("[STATE] Tax receipt processed successfully!");
            println!("  Receipt hash: {}", hex::encode(receipt_hash));
            println!("  Gov log version: {}", payment.gov_log_version);
            println!("  Coins credited: {}", coins_transferred);
            println!("  New balance: {} coins", self.coins);
        } else {
            println!("[WARN] No matching tax payment found for receipt!");
            println!("  Either payment doesn't exist or receipt already applied");
        }
    }
    
    /// Check if tax payment is needed for a government log version
    pub fn needs_tax_payment(&self, gov_log_version: u64) -> bool {
        !self.paid_log_versions.get(&gov_log_version).copied().unwrap_or(false)
    }
    
    /// Get tax payment status summary
    pub fn get_tax_status(&self) -> String {
        let mut status = String::from("Tax Payment Status:\n");
        
        let mut versions: Vec<u64> = self.paid_log_versions.keys().copied().collect();
        versions.sort();
        
        if versions.is_empty() {
            status.push_str("  No tax payments recorded\n");
        } else {
            for version in versions {
                if let Some(payment) = self.tax_payments.iter()
                    .find(|p| p.gov_log_version == version)
                {
                    let receipt_status = if payment.receipt_hash.is_some() {
                        "CONFIRMED"
                    } else {
                        "PENDING"
                    };
                    status.push_str(&format!(
                        "  Version {}: {} coins - {}\n",
                        version, payment.amount, receipt_status
                    ));
                }
            }
        }
        
        status
    }
    
    /// Get current state summary
    pub fn get_summary(&self) -> String {
        format!(
            "Resources: Land={}, Labor={}, Capital={} | Coins: {} | Active tasks: {} | Completed: {} | Tax payments: {}",
            self.resources.land,
            self.resources.labor,
            self.resources.capital,
            self.coins,
            self.active_tasks.len(),
            self.completed_tasks.len(),
            self.tax_payments.len()
        )
    }
}

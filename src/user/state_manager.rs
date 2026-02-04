use crate::types::*;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct UserStateManager {
    pub active_tasks: HashMap<String, Task>,
    pub resources: Resources,
    pub coins: u64,
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
        }
    }
    
    pub fn start_task(&mut self, task_name: String, rule: &ActionRule) -> Result<(), String> {
        if self.active_tasks.contains_key(&task_name) {
            return Err("Task already active".to_string());
        }
        
        if self.resources.land < rule.required_resources.land {
            return Err("Insufficient land".to_string());
        }
        if self.resources.labor < rule.required_resources.labor {
            return Err("Insufficient labor".to_string());
        }
        if self.resources.capital < rule.required_resources.capital {
            return Err("Insufficient capital".to_string());
        }
        
        self.resources.land -= rule.required_resources.land;
        self.resources.labor -= rule.required_resources.labor;
        self.resources.capital -= rule.required_resources.capital;
        
        let task = Task {
            task_name: task_name.clone(),
            start_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            duration: 60,
            resources_locked: rule.required_resources.clone(),
        };
        
        self.active_tasks.insert(task_name, task);
        
        Ok(())
    }
    
    pub fn harvest_task(&mut self, task_name: &str, multiplier: u64) -> Result<u64, String> {
        let task = self.active_tasks.remove(task_name)
            .ok_or_else(|| "Task not found".to_string())?;
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if current_time < task.start_time + task.duration {
            self.active_tasks.insert(task_name.to_string(), task);
            return Err("Task not complete yet".to_string());
        }
        
        self.resources.land += task.resources_locked.land;
        self.resources.labor += task.resources_locked.labor;
        self.resources.capital += task.resources_locked.capital;
        
        let coins_earned = multiplier;
        self.coins += coins_earned;
        
        Ok(coins_earned)
    }
}

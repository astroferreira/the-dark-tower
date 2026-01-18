//! Workplace production management
//!
//! Handles production queues and workplace management.

use std::collections::HashMap;

use crate::simulation::types::TileCoord;
use crate::simulation::workplaces::types::{Workplace, WorkplaceId, WorkplaceType, WorkOrder};
use crate::simulation::jobs::types::JobType;

/// Manager for workplaces
#[derive(Clone, Debug, Default)]
pub struct WorkplaceManager {
    /// All workplaces
    pub workplaces: HashMap<WorkplaceId, Workplace>,
    /// Production queue
    pub work_orders: Vec<WorkOrder>,
    /// Next workplace ID
    next_id: u64,
    /// Next work order ID
    next_order_id: u64,
}

impl WorkplaceManager {
    pub fn new() -> Self {
        WorkplaceManager {
            workplaces: HashMap::new(),
            work_orders: Vec::new(),
            next_id: 0,
            next_order_id: 0,
        }
    }

    /// Create a new workplace
    pub fn create_workplace(&mut self, workplace_type: WorkplaceType, location: TileCoord) -> WorkplaceId {
        let id = WorkplaceId(self.next_id);
        self.next_id += 1;
        self.workplaces.insert(id, Workplace::new(id, workplace_type, location));
        id
    }

    /// Get a workplace by ID
    pub fn get(&self, id: WorkplaceId) -> Option<&Workplace> {
        self.workplaces.get(&id)
    }

    /// Get mutable workplace by ID
    pub fn get_mut(&mut self, id: WorkplaceId) -> Option<&mut Workplace> {
        self.workplaces.get_mut(&id)
    }

    /// Get workplaces at a location
    pub fn at_location(&self, location: &TileCoord) -> Vec<&Workplace> {
        self.workplaces.values()
            .filter(|wp| &wp.location == location)
            .collect()
    }

    /// Get workplaces of a type
    pub fn of_type(&self, workplace_type: WorkplaceType) -> Vec<&Workplace> {
        self.workplaces.values()
            .filter(|wp| wp.workplace_type == workplace_type)
            .collect()
    }

    /// Get workplaces that support a job
    pub fn for_job(&self, job_type: JobType) -> Vec<&Workplace> {
        self.workplaces.values()
            .filter(|wp| wp.workplace_type.supported_jobs().contains(&job_type))
            .collect()
    }

    /// Get total workers at all workplaces
    pub fn total_workers(&self) -> u32 {
        self.workplaces.values().map(|wp| wp.current_workers).sum()
    }

    /// Get total capacity of all workplaces
    pub fn total_capacity(&self) -> u32 {
        self.workplaces.values()
            .map(|wp| wp.workplace_type.max_workers())
            .sum()
    }

    /// Get capacity for a job type
    pub fn capacity_for_job(&self, job_type: JobType) -> u32 {
        self.for_job(job_type)
            .iter()
            .map(|wp| wp.workplace_type.max_workers())
            .sum()
    }

    /// Get available capacity for a job type
    pub fn available_capacity_for_job(&self, job_type: JobType) -> u32 {
        self.for_job(job_type)
            .iter()
            .map(|wp| wp.available_slots())
            .sum()
    }

    /// Create a work order
    pub fn create_work_order(
        &mut self,
        workplace_id: WorkplaceId,
        job_type: JobType,
        target: f32,
    ) -> Option<u64> {
        // Verify workplace exists and supports this job
        let workplace = self.workplaces.get(&workplace_id)?;
        if !workplace.workplace_type.supported_jobs().contains(&job_type) {
            return None;
        }

        let id = self.next_order_id;
        self.next_order_id += 1;

        self.work_orders.push(WorkOrder::new(id, workplace_id, job_type, target));
        Some(id)
    }

    /// Get pending work orders for a job type
    pub fn pending_orders_for_job(&self, job_type: JobType) -> Vec<&WorkOrder> {
        self.work_orders.iter()
            .filter(|o| o.job_type == job_type && !o.is_complete())
            .collect()
    }

    /// Update work order progress
    pub fn add_order_progress(&mut self, order_id: u64, progress: f32) {
        if let Some(order) = self.work_orders.iter_mut().find(|o| o.id == order_id) {
            order.add_progress(progress);
        }
    }

    /// Remove completed work orders
    pub fn cleanup_completed_orders(&mut self) {
        self.work_orders.retain(|o| !o.is_complete());
    }

    /// Get average efficiency for a job type
    pub fn average_efficiency_for_job(&self, job_type: JobType) -> f32 {
        let workplaces: Vec<_> = self.for_job(job_type);
        if workplaces.is_empty() {
            return 1.0;
        }

        let total_eff: f32 = workplaces.iter().map(|wp| wp.efficiency).sum();
        total_eff / workplaces.len() as f32
    }

    /// Assign workers to workplaces for a job
    pub fn assign_workers_to_job(&mut self, job_type: JobType, count: u32) -> u32 {
        let mut assigned = 0;
        let mut remaining = count;

        // Get workplace IDs that support this job
        let workplace_ids: Vec<_> = self.workplaces.iter()
            .filter(|(_, wp)| wp.workplace_type.supported_jobs().contains(&job_type))
            .map(|(id, _)| *id)
            .collect();

        for id in workplace_ids {
            if remaining == 0 {
                break;
            }

            if let Some(workplace) = self.workplaces.get_mut(&id) {
                if workplace.can_accept_workers() {
                    let added = workplace.add_workers(remaining);
                    assigned += added;
                    remaining -= added;
                }
            }
        }

        assigned
    }

    /// Clear all workers from workplaces
    pub fn clear_workers(&mut self) {
        for workplace in self.workplaces.values_mut() {
            workplace.current_workers = 0;
        }
    }

    /// Get workplace summary
    pub fn summary(&self) -> WorkplaceSummary {
        WorkplaceSummary {
            total_workplaces: self.workplaces.len(),
            total_workers: self.total_workers(),
            total_capacity: self.total_capacity(),
            pending_orders: self.work_orders.iter().filter(|o| !o.is_complete()).count(),
        }
    }
}

/// Summary of workplace state
#[derive(Clone, Debug)]
pub struct WorkplaceSummary {
    pub total_workplaces: usize,
    pub total_workers: u32,
    pub total_capacity: u32,
    pub pending_orders: usize,
}

impl WorkplaceSummary {
    pub fn utilization(&self) -> f32 {
        if self.total_capacity == 0 {
            0.0
        } else {
            self.total_workers as f32 / self.total_capacity as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workplace_manager() {
        let mut manager = WorkplaceManager::new();
        let id = manager.create_workplace(WorkplaceType::Farm, TileCoord::new(10, 10));

        assert!(manager.get(id).is_some());
        assert_eq!(manager.total_capacity(), 10); // Farm capacity
    }

    #[test]
    fn test_worker_assignment() {
        let mut manager = WorkplaceManager::new();
        manager.create_workplace(WorkplaceType::Farm, TileCoord::new(10, 10));

        let assigned = manager.assign_workers_to_job(JobType::Farmer, 5);
        assert_eq!(assigned, 5);
        assert_eq!(manager.total_workers(), 5);
    }

    #[test]
    fn test_work_orders() {
        let mut manager = WorkplaceManager::new();
        let wp_id = manager.create_workplace(WorkplaceType::Farm, TileCoord::new(10, 10));

        let order_id = manager.create_work_order(wp_id, JobType::Farmer, 100.0);
        assert!(order_id.is_some());

        manager.add_order_progress(order_id.unwrap(), 100.0);
        assert_eq!(manager.pending_orders_for_job(JobType::Farmer).len(), 0);
    }
}

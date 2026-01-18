//! Leadership succession handling
//!
//! Handles leader death, succession crises, and transitions of power.

use rand::Rng;
use crate::simulation::society::types::{SocietyState, SocietyType, SuccessionMethod};

/// Process succession for a tribe
/// Returns true if succession completed successfully
pub fn process_succession<R: Rng>(
    society_state: &mut SocietyState,
    notable_colonist_names: &[String],
    rng: &mut R,
) -> bool {
    if !society_state.in_succession_crisis {
        return true;
    }

    // If crisis is over, select new leader
    if society_state.succession_crisis_ticks == 0 {
        let (name, age) = select_new_leader(
            &society_state.society_type,
            notable_colonist_names,
            rng,
        );
        society_state.set_leader(None, name, age);
        return true;
    }

    false
}

/// Select a new leader based on succession method
fn select_new_leader<R: Rng>(
    society_type: &SocietyType,
    notable_names: &[String],
    rng: &mut R,
) -> (String, u32) {
    let config = society_type.config();

    match config.succession {
        SuccessionMethod::Hereditary => {
            // Choose from existing notables (heir)
            let name = if !notable_names.is_empty() && rng.gen_bool(0.7) {
                notable_names[rng.gen_range(0..notable_names.len())].clone()
            } else {
                generate_leader_name(rng)
            };
            let age = 18 + rng.gen_range(0..20); // Young heir
            (name, age)
        }
        SuccessionMethod::Divine => {
            // Priest chosen by signs
            let name = format!("High Priest {}", generate_leader_name(rng));
            let age = 30 + rng.gen_range(0..30); // Middle-aged to old
            (name, age)
        }
        SuccessionMethod::Election => {
            // Popular election
            let name = if !notable_names.is_empty() {
                notable_names[rng.gen_range(0..notable_names.len())].clone()
            } else {
                generate_leader_name(rng)
            };
            let age = 35 + rng.gen_range(0..25); // Mature
            (name, age)
        }
        SuccessionMethod::ElderCouncil => {
            // Elders choose wisest
            let name = format!("Elder {}", generate_leader_name(rng));
            let age = 50 + rng.gen_range(0..20); // Old and wise
            (name, age)
        }
        SuccessionMethod::Coup => {
            // Military strongman
            let name = format!("General {}", generate_leader_name(rng));
            let age = 30 + rng.gen_range(0..20); // In their prime
            (name, age)
        }
        SuccessionMethod::WealthElection => {
            // Richest merchant
            let name = format!("Merchant Prince {}", generate_leader_name(rng));
            let age = 40 + rng.gen_range(0..25);
            (name, age)
        }
    }
}

/// Generate a random leader name
fn generate_leader_name<R: Rng>(rng: &mut R) -> String {
    let first_names = [
        "Aldric", "Beren", "Cadmus", "Doran", "Edmund", "Falk", "Gareth", "Harald",
        "Ingvar", "Jorund", "Kael", "Leofric", "Magnus", "Nils", "Osric", "Ragnar",
        "Sigurd", "Theron", "Ulric", "Valdis", "Wulfric", "Yngvar", "Zoran", "Aeric",
        "Brynn", "Cormac", "Derrick", "Egon", "Finn", "Godfrey", "Halvar", "Ivan",
        "Jasper", "Kellan", "Lars", "Milo", "Niall", "Odin", "Pierce", "Quinn",
    ];

    let surnames = [
        "Ironhand", "Stoneheart", "Goldmantle", "Silverbrow", "Blackwood", "Redmane",
        "Whitestorm", "Greywolf", "Darkwater", "Brightforge", "Swiftarrow", "Strongbow",
        "Firebrand", "Frostborn", "Thunderhelm", "Shadowbane", "Lightbringer", "Dawnguard",
        "Nightfall", "Stormwind", "Riverdale", "Mountaincrest", "Valleyford", "Oakenshield",
    ];

    format!(
        "{} {}",
        first_names[rng.gen_range(0..first_names.len())],
        surnames[rng.gen_range(0..surnames.len())]
    )
}

/// Check if a leader should die of old age
pub fn check_leader_death<R: Rng>(society_state: &SocietyState, rng: &mut R) -> bool {
    let age = society_state.leader_age;

    // Base death chance increases with age
    let death_chance = if age < 40 {
        0.001 // Very low
    } else if age < 60 {
        0.01 // Low
    } else if age < 70 {
        0.03 // Moderate
    } else if age < 80 {
        0.08 // High
    } else {
        0.15 // Very high
    };

    rng.gen::<f32>() < death_chance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_leader_name() {
        let mut rng = rand::thread_rng();
        let name = generate_leader_name(&mut rng);
        assert!(!name.is_empty());
        assert!(name.contains(' ')); // First and last name
    }

    #[test]
    fn test_select_new_leader() {
        let mut rng = rand::thread_rng();
        let notables = vec!["John Smith".to_string(), "Jane Doe".to_string()];

        for society_type in SocietyType::all() {
            let (name, age) = select_new_leader(society_type, &notables, &mut rng);
            assert!(!name.is_empty());
            assert!(age >= 18 && age < 100);
        }
    }
}

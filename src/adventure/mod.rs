//! Adventure mode: one person of the world, from nobody to a name in its history.
//!
//! DF's adventure mode is the model for the world's side: the places are the history's (towns,
//! ruins, beasts' lairs, tombs of named dead, gods' temples, cults' shrines, the Shadow's seat),
//! each realized on demand from its seed and its cause; skills rise by use; the townsfolk know
//! what their town has heard; treasures of the history lie where they were lost. Tibia is the
//! model for the hero's side: levels from experience, a calling at level 8 (knight, paladin,
//! sorcerer, druid) with its spells, loot from what is slain, floors joined by stairs, ladders,
//! holes and ropes, keys and levers, quest chests that give one reward, bounties, death that
//! costs experience and sends you back to the temple.
//!
//! The simulation is here and deterministic (`Game::act`); the window is `tiles::adventure`.

pub mod actor;
pub mod bot;
pub mod data;
pub mod game;
pub mod hero;
pub mod item;
pub mod land;
pub mod living;
pub mod lore;
pub mod map;
pub mod npc;
pub mod people;
pub mod quest;
pub mod regard;
pub mod rooms;
pub mod site;
pub mod surface;
pub mod tales;
pub mod town;
pub mod wonders;
pub mod world;

pub use game::{Action, Game};

/// A new adventure on this world: the hero is of the start town's people.
pub fn new_game(world: &crate::world::WorldData, history: Option<&crate::history::world_state::WorldHistory>, seed: u64, name: Option<&str>) -> Game {
    let built = world::build(world, history, seed, None);
    let start = built.start;
    let people = built.sites.iter().find(|s| s.id == start).map(|s| s.people.clone()).filter(|p| !p.is_empty()).unwrap_or_else(|| "human".into());
    let female = seed % 2 == 1;
    let name = name.map(|n| n.to_string()).unwrap_or_else(|| town::person_name(&people, seed ^ 0xBEEF));
    let hero = hero::Hero::new(&name, &people, female, seed);
    Game::new(built.info, built.sites, hero, start, seed)
}

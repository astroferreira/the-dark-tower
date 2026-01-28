//! Legends mode state machine.
//!
//! Manages navigation between category list, entity list, and detail views.

use crate::history::world_state::WorldHistory;
use super::queries::{Category, EntityRef, ListEntry, EntityDetail, list_entities, entity_detail};

/// Current view state in legends mode.
#[derive(Clone, Debug)]
pub enum LegendsView {
    /// Top-level category selection
    CategoryMenu,
    /// Browsing a list of entities within a category
    EntityList {
        category: Category,
        entries: Vec<ListEntry>,
        selected: usize,
        scroll: usize,
        search: String,
    },
    /// Viewing details of a single entity
    EntityDetail {
        category: Category,
        detail: EntityDetail,
        scroll: usize,
        /// Cursor line index for navigating links
        selected: usize,
        /// The entity reference for back-navigation context
        entity: EntityRef,
    },
}

/// The legends mode controller.
pub struct LegendsMode {
    pub view: LegendsView,
    pub category_selected: usize,
    /// Navigation stack for back-navigation through linked entities.
    pub nav_stack: Vec<LegendsView>,
}

impl LegendsMode {
    pub fn new() -> Self {
        Self {
            view: LegendsView::CategoryMenu,
            category_selected: 0,
            nav_stack: Vec::new(),
        }
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        match &mut self.view {
            LegendsView::CategoryMenu => {
                if self.category_selected > 0 {
                    self.category_selected -= 1;
                }
            }
            LegendsView::EntityList { selected, scroll, .. } => {
                if *selected > 0 {
                    *selected -= 1;
                    if *selected < *scroll {
                        *scroll = *selected;
                    }
                }
            }
            LegendsView::EntityDetail { selected, scroll, .. } => {
                if *selected > 0 {
                    *selected -= 1;
                    // Auto-scroll to keep cursor visible
                    if *selected < *scroll {
                        *scroll = *selected;
                    }
                }
            }
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self, visible_height: usize) {
        match &mut self.view {
            LegendsView::CategoryMenu => {
                let max = Category::all().len().saturating_sub(1);
                if self.category_selected < max {
                    self.category_selected += 1;
                }
            }
            LegendsView::EntityList { selected, scroll, entries, .. } => {
                if *selected + 1 < entries.len() {
                    *selected += 1;
                    if *selected >= *scroll + visible_height {
                        *scroll = selected.saturating_sub(visible_height - 1);
                    }
                }
            }
            LegendsView::EntityDetail { selected, scroll, detail, .. } => {
                if *selected + 1 < detail.lines.len() {
                    *selected += 1;
                    // Auto-scroll to keep cursor visible
                    if *selected >= *scroll + visible_height {
                        *scroll = selected.saturating_sub(visible_height - 1);
                    }
                }
            }
        }
    }

    /// Select current item (enter a category or view entity detail).
    pub fn select(&mut self, history: &WorldHistory) {
        match &self.view {
            LegendsView::CategoryMenu => {
                let categories = Category::all();
                if self.category_selected < categories.len() {
                    let category = categories[self.category_selected];
                    let entries = list_entities(history, category, "");
                    let prev = std::mem::replace(&mut self.view, LegendsView::CategoryMenu);
                    self.nav_stack.push(prev);
                    self.view = LegendsView::EntityList {
                        category,
                        entries,
                        selected: 0,
                        scroll: 0,
                        search: String::new(),
                    };
                }
            }
            LegendsView::EntityList { category, entries, selected, .. } => {
                if *selected < entries.len() {
                    let entry = &entries[*selected];
                    let detail = entity_detail(history, &entry.id);
                    let cat = *category;
                    let entity_ref = entry.id.clone();
                    let prev = std::mem::replace(&mut self.view, LegendsView::CategoryMenu);
                    self.nav_stack.push(prev);
                    self.view = LegendsView::EntityDetail {
                        category: cat,
                        detail,
                        scroll: 0,
                        selected: 0,
                        entity: entity_ref,
                    };
                }
            }
            LegendsView::EntityDetail { .. } => {
                self.navigate_to_link(history);
            }
        }
    }

    /// Navigate into the entity linked on the current cursor line.
    pub fn navigate_to_link(&mut self, history: &WorldHistory) {
        let link = if let LegendsView::EntityDetail { detail, selected, .. } = &self.view {
            detail.lines.get(*selected).and_then(|line| line.link.clone())
        } else {
            None
        };

        if let Some(entity_ref) = link {
            let detail = entity_detail(history, &entity_ref);
            let cat = entity_ref.category();
            let prev = std::mem::replace(&mut self.view, LegendsView::CategoryMenu);
            self.nav_stack.push(prev);
            self.view = LegendsView::EntityDetail {
                category: cat,
                detail,
                scroll: 0,
                selected: 0,
                entity: entity_ref,
            };
        }
    }

    /// Return the link on the current cursor line, if any.
    pub fn current_link(&self) -> Option<&EntityRef> {
        if let LegendsView::EntityDetail { detail, selected, .. } = &self.view {
            detail.lines.get(*selected).and_then(|line| line.link.as_ref())
        } else {
            None
        }
    }

    /// Go back one level (pop from navigation stack).
    pub fn go_back(&mut self) {
        if let Some(prev) = self.nav_stack.pop() {
            self.view = prev;
        }
    }

    /// Update the search filter and refresh the entity list.
    pub fn update_search(&mut self, history: &WorldHistory, search: String) {
        if let LegendsView::EntityList { category, entries, selected, scroll, .. } = &mut self.view {
            let new_entries = list_entities(history, *category, &search);
            *entries = new_entries;
            *selected = 0;
            *scroll = 0;
            // Update the search field
            let cat = *category;
            self.view = LegendsView::EntityList {
                category: cat,
                entries: entries.clone(),
                selected: 0,
                scroll: 0,
                search,
            };
        }
    }

    /// Add a character to the search.
    pub fn search_push(&mut self, history: &WorldHistory, ch: char) {
        if let LegendsView::EntityList { search, .. } = &self.view {
            let mut new_search = search.clone();
            new_search.push(ch);
            self.update_search(history, new_search);
        }
    }

    /// Remove last character from search.
    pub fn search_pop(&mut self, history: &WorldHistory) {
        if let LegendsView::EntityList { search, .. } = &self.view {
            let mut new_search = search.clone();
            new_search.pop();
            self.update_search(history, new_search);
        }
    }

    /// Get the current view title.
    pub fn title(&self) -> String {
        match &self.view {
            LegendsView::CategoryMenu => "Legends - Browse History".to_string(),
            LegendsView::EntityList { category, search, entries, .. } => {
                if search.is_empty() {
                    format!("Legends - {} ({} entries)", category.name(), entries.len())
                } else {
                    format!("Legends - {} [search: {}] ({} matches)", category.name(), search, entries.len())
                }
            }
            LegendsView::EntityDetail { detail, .. } => {
                format!("Legends - {}", detail.title)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::config::HistoryConfig;
    use crate::history::simulation::HistoryEngine;
    use crate::tilemap::Tilemap;
    use crate::biomes::ExtendedBiome;
    use crate::seeds::WorldSeeds;
    use crate::scale::MapScale;
    use crate::plates::PlateId;
    use crate::water_bodies::WaterBodyId;
    use crate::world::WorldData;

    fn make_test_world() -> WorldData {
        let width = 64;
        let height = 32;
        let mut heightmap = Tilemap::new_with(width, height, 0.3);
        let mut biomes = Tilemap::new_with(width, height, ExtendedBiome::TemperateGrassland);
        for x in 0..width {
            *biomes.get_mut(x, 0) = ExtendedBiome::Ocean;
            *heightmap.get_mut(x, 0) = -0.1;
        }
        let seeds = WorldSeeds::from_master(42);
        let scale = MapScale::new(1.0);
        let temperature = Tilemap::new_with(width, height, 15.0);
        let moisture = Tilemap::new_with(width, height, 0.5);
        let stress_map = Tilemap::new_with(width, height, 0.0);
        let plate_map = Tilemap::new_with(width, height, PlateId(0));
        let water_body_map = Tilemap::new_with(width, height, WaterBodyId::NONE);
        let water_depth = Tilemap::new_with(width, height, 0.0);
        WorldData::new(
            seeds, scale, heightmap, temperature, moisture,
            biomes, stress_map, plate_map, Vec::new(),
            None, water_body_map, Vec::new(), water_depth,
            None, None,
        )
    }

    #[test]
    fn test_legends_navigation() {
        let world = make_test_world();
        let config = HistoryConfig {
            simulation_years: 50,
            initial_civilizations: 3,
            ..HistoryConfig::default()
        };
        let mut engine = HistoryEngine::new(42);
        let history = engine.simulate(&world, config);

        let mut mode = LegendsMode::new();

        // Start at category menu
        assert!(matches!(mode.view, LegendsView::CategoryMenu));
        assert_eq!(mode.category_selected, 0);

        // Navigate down
        mode.move_down(20);
        assert_eq!(mode.category_selected, 1);

        // Select first category (Factions) — pushes CategoryMenu onto stack
        mode.category_selected = 0;
        mode.select(&history);
        assert!(matches!(mode.view, LegendsView::EntityList { .. }));
        assert_eq!(mode.nav_stack.len(), 1);

        // Select first entity — pushes EntityList onto stack
        mode.select(&history);
        assert!(matches!(mode.view, LegendsView::EntityDetail { .. }));
        assert_eq!(mode.nav_stack.len(), 2);

        // Go back pops to EntityList
        mode.go_back();
        assert!(matches!(mode.view, LegendsView::EntityList { .. }));
        assert_eq!(mode.nav_stack.len(), 1);

        // Go back pops to CategoryMenu
        mode.go_back();
        assert!(matches!(mode.view, LegendsView::CategoryMenu));
        assert_eq!(mode.nav_stack.len(), 0);
    }
}

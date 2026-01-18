//! Trade system between tribes

use rand::Rng;

use crate::simulation::types::{TribeId, ResourceType, TribeEventType, TreatyType};
use crate::simulation::params::SimulationParams;
use crate::simulation::simulation::SimulationState;

/// Process trade opportunities for a tick
pub fn process_trade_tick<R: Rng>(
    state: &mut SimulationState,
    params: &SimulationParams,
    rng: &mut R,
) {
    let tribe_ids: Vec<TribeId> = state.tribes.keys().copied().collect();

    for &tribe_a in &tribe_ids {
        // Get tribe info
        let (culture_a, neighbors) = {
            let tribe = match state.tribes.get(&tribe_a) {
                Some(t) if t.is_alive => t,
                _ => continue,
            };
            (tribe.culture.clone(), state.neighboring_tribes(tribe_a))
        };

        for &tribe_b in &neighbors {
            // Check if tribe_b is alive
            let tribe_b_alive = state.tribes.get(&tribe_b).map(|t| t.is_alive).unwrap_or(false);
            if !tribe_b_alive {
                continue;
            }

            // Get relation
            let relation = state.diplomacy.get_relation(tribe_a, tribe_b);

            // Culture check for trade willingness
            if !culture_a.will_consider_trade(relation.0, rng) {
                continue;
            }

            // Trade agreement bonus
            let has_trade_agreement = state.diplomacy.has_treaty(tribe_a, tribe_b, TreatyType::TradeAgreement);
            let trade_chance = if has_trade_agreement { 0.3 } else { 0.1 };

            if rng.gen::<f32>() > trade_chance {
                continue;
            }

            // Try to find a mutually beneficial trade
            if let Some(trade) = find_trade_opportunity(state, tribe_a, tribe_b) {
                execute_trade(state, tribe_a, tribe_b, trade, params);
            }
        }
    }
}

/// A potential trade between tribes
struct TradeOffer {
    give_resource: ResourceType,
    give_amount: f32,
    receive_resource: ResourceType,
    receive_amount: f32,
}

/// Find a mutually beneficial trade between two tribes
fn find_trade_opportunity(
    state: &SimulationState,
    tribe_a: TribeId,
    tribe_b: TribeId,
) -> Option<TradeOffer> {
    let (stockpile_a, culture_a) = {
        let tribe = state.tribes.get(&tribe_a)?;
        (tribe.stockpile.clone(), tribe.culture.clone())
    };

    let (stockpile_b, culture_b) = {
        let tribe = state.tribes.get(&tribe_b)?;
        (tribe.stockpile.clone(), tribe.culture.clone())
    };

    // Find what A has surplus of and B needs
    let a_surplus = find_surplus_resource(&stockpile_a);
    let b_needs = find_needed_resource(&stockpile_b, &culture_b);

    // Find what B has surplus of and A needs
    let b_surplus = find_surplus_resource(&stockpile_b);
    let a_needs = find_needed_resource(&stockpile_a, &culture_a);

    // Check if there's a match
    if let (Some((a_give, a_give_amt)), Some((b_want, _))) = (a_surplus, b_needs) {
        if a_give == b_want {
            if let (Some((b_give, b_give_amt)), Some((a_want, _))) = (b_surplus, a_needs) {
                if b_give == a_want {
                    // Calculate fair exchange based on trade values
                    let a_value = a_give.trade_value() * a_give_amt;
                    let b_value = b_give.trade_value() * b_give_amt;

                    // Normalize to similar values
                    let (final_a_amt, final_b_amt) = if a_value > b_value {
                        (a_give_amt * (b_value / a_value), b_give_amt)
                    } else {
                        (a_give_amt, b_give_amt * (a_value / b_value))
                    };

                    return Some(TradeOffer {
                        give_resource: a_give,
                        give_amount: final_a_amt.min(10.0), // Cap trade size
                        receive_resource: b_give,
                        receive_amount: final_b_amt.min(10.0),
                    });
                }
            }
        }
    }

    None
}

/// Find a resource the tribe has surplus of
fn find_surplus_resource(stockpile: &crate::simulation::resources::Stockpile) -> Option<(ResourceType, f32)> {
    let tradeable = [
        ResourceType::Food,
        ResourceType::Wood,
        ResourceType::Stone,
        ResourceType::Leather,
        ResourceType::Cloth,
        ResourceType::Salt,
        ResourceType::Copper,
        ResourceType::Iron,
        ResourceType::Gold,
    ];

    for &resource in &tradeable {
        let amount = stockpile.get(resource);
        let capacity = stockpile.get_capacity(resource);

        // Has at least 30% of capacity = surplus
        if amount > capacity * 0.3 && amount > 5.0 {
            return Some((resource, (amount - capacity * 0.2).max(1.0)));
        }
    }

    None
}

/// Find a resource the tribe needs
fn find_needed_resource(
    stockpile: &crate::simulation::resources::Stockpile,
    culture: &crate::simulation::tribe::TribeCulture,
) -> Option<(ResourceType, f32)> {
    // Prioritize culturally valued resources
    for &resource in &culture.valued_resources {
        let amount = stockpile.get(resource);
        let capacity = stockpile.get_capacity(resource);

        if amount < capacity * 0.2 {
            return Some((resource, capacity * 0.3 - amount));
        }
    }

    // Then check basic needs
    let basic = [ResourceType::Food, ResourceType::Water, ResourceType::Wood];
    for &resource in &basic {
        let amount = stockpile.get(resource);
        let capacity = stockpile.get_capacity(resource);

        if amount < capacity * 0.3 {
            return Some((resource, capacity * 0.3 - amount));
        }
    }

    None
}

/// Execute a trade between two tribes
fn execute_trade(
    state: &mut SimulationState,
    tribe_a: TribeId,
    tribe_b: TribeId,
    trade: TradeOffer,
    params: &SimulationParams,
) {
    // Remove resources from A, add to B
    if let Some(tribe) = state.tribes.get_mut(&tribe_a) {
        tribe.stockpile.remove(trade.give_resource, trade.give_amount);
        tribe.stockpile.add(trade.receive_resource, trade.receive_amount);
        tribe.record_event(
            state.current_tick,
            TribeEventType::TradeCompleted {
                with: tribe_b,
                gave: vec![(trade.give_resource, trade.give_amount)],
                received: vec![(trade.receive_resource, trade.receive_amount)],
            },
        );
    }

    // Remove resources from B, add to A
    if let Some(tribe) = state.tribes.get_mut(&tribe_b) {
        tribe.stockpile.remove(trade.receive_resource, trade.receive_amount);
        tribe.stockpile.add(trade.give_resource, trade.give_amount);
        tribe.record_event(
            state.current_tick,
            TribeEventType::TradeCompleted {
                with: tribe_a,
                gave: vec![(trade.receive_resource, trade.receive_amount)],
                received: vec![(trade.give_resource, trade.give_amount)],
            },
        );
    }

    // Improve relations
    state.diplomacy.adjust_relation(tribe_a, tribe_b, params.trade_relation_boost);
    state.stats.total_trades += 1;
}

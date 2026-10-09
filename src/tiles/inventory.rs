//! `--inventory PREFIX`: every kind the simulation defines, rendered in the game's own frame.
//!
//! A test camp on the dev embark: every built work raised through the colony's own
//! `project_step` (the last load laid), every mark, founding stone, creature kind and settler
//! state set, the relic, a prisoner, a mandate, a siege camp, the clash, the bell, an expedition
//! away, a snatched child among returning raiders. The camp is drawn by `render_local` +
//! `draw_colony` exactly as the window draws it, and each kind is cropped from that frame into a
//! labelled contact sheet (`<prefix>_inventory.png`); the cave and level kinds come from a level
//! frame. It prints what could not be placed, so nothing is claimed that was not drawn.

use super::render::{render_local, LocalCamera};
use super::viewer::{colony_site, found_colony};
use crate::colony::creatures::{Creature, CreatureKind};
use crate::colony::projects::{Project, ProjectKind};
use crate::colony::{ColonyMark, Item, ItemKind, MarkKind, StoneKind};
use crate::world::WorldData;

pub fn save_inventory(world: &WorldData, history: Option<&crate::history::world_state::WorldHistory>, atlas: &super::Atlas, tile: (usize, usize), prefix: &str) -> Result<(usize, Vec<String>), Box<dyn std::error::Error>> {
    let cell = None;
    let (map, seed, _) = colony_site(world, history, tile, cell);
    let mut c = found_colony(map, history, tile, seed, 11);
    // Two hundred days on (clothes 180 days worn are rags), at noon in summer.
    c.clock.tick += 200 * crate::colony::TICKS_PER_DAY;
    let to_noon = (12 * 60 + crate::colony::TICKS_PER_DAY - c.clock.tick % crate::colony::TICKS_PER_DAY) % crate::colony::TICKS_PER_DAY;
    c.clock.tick += to_noon;
    let mut missing: Vec<String> = Vec::new();
    // (label, cell to crop round, crop half-size in cells)
    let mut shots: Vec<(String, (f32, f32), f32)> = Vec::new();
    let tick0 = c.clock.tick;

    // Every built work, raised by the colony's own completion code.
    let works: [(ProjectKind, ItemKind); 26] = [
        (ProjectKind::SecondHut, ItemKind::Log), (ProjectKind::Storehouse, ItemKind::Log), (ProjectKind::Workshop, ItemKind::Log), (ProjectKind::Temple, ItemKind::Stone),
        (ProjectKind::LordsHall, ItemKind::Stone), (ProjectKind::Tavern, ItemKind::Log), (ProjectKind::GuildHall, ItemKind::Log), (ProjectKind::Kitchen, ItemKind::Stone),
        (ProjectKind::Library, ItemKind::Log), (ProjectKind::Smokehouse, ItemKind::Log), (ProjectKind::DryingRack, ItemKind::Log), (ProjectKind::Still, ItemKind::Log),
        (ProjectKind::Well, ItemKind::Stone), (ProjectKind::Windbreak, ItemKind::Stone), (ProjectKind::Fence, ItemKind::Log), (ProjectKind::Field, ItemKind::Log),
        (ProjectKind::Pen, ItemKind::Log), (ProjectKind::Lookout, ItemKind::Log), (ProjectKind::Jetty, ItemKind::Log), (ProjectKind::Palisade, ItemKind::Log),
        (ProjectKind::Traps, ItemKind::Log), (ProjectKind::Woodshed, ItemKind::Log), (ProjectKind::Mending, ItemKind::Log), (ProjectKind::Lining, ItemKind::Stone),
        (ProjectKind::Hatch, ItemKind::Log), (ProjectKind::Woodpile, ItemKind::Log),
    ];
    let builder = c.settlers.iter().position(|s| s.alive).unwrap_or(0);
    for (kind, mat) in works {
        let fp = crate::colony::Colony::footprint(kind).or(if kind == ProjectKind::Lookout { Some((2, 2)) } else { None });
        let at = match (kind, fp) {
            (ProjectKind::Palisade | ProjectKind::Mending | ProjectKind::Traps, _) => c.camp,
            (_, Some((w, h))) => match c.find_site_for(kind, w, h) { Some(p) => p, None => { missing.push(format!("{:?}: no site", kind)); continue } },
            _ => c.camp,
        };
        let day = c.clock.day();
        c.projects.push(Project { kind, at, needed: 1, used: 0, material: mat, why: "for the inventory".into(), done: false, day });
        c.items.push(Item::new(mat, c.camp, true));
        // The work must be the one under way: lay its load until it stands.
        let k = c.projects.len() - 1;
        for _ in 0..3 { if c.projects[k].done { break; } c.project_step(builder); }
        if !c.projects[k].done { missing.push(format!("{:?}: not finished by project_step", kind)); continue; }
        if let Some((w, h)) = fp { shots.push((format!("{:?}", kind), (at.0 as f32 + w as f32 / 2.0, at.1 as f32 + h as f32 / 2.0), (w.max(h) as f32 / 2.0 + 2.0).max(3.5))); }
    }
    if let Some(p) = c.projects.iter().find(|p| p.kind == ProjectKind::Palisade && p.done) { shots.push(("Palisade".into(), (p.at.0 as f32 + c.wall_r() as f32, p.at.1 as f32 + 0.5), 4.0)); }
    // A pen needs beasts in it.
    if c.pen.is_none() { c.pen = Some(("wild boar".into(), 3)); }

    // Founding stones and every mark.
    let spot = |c: &crate::colony::Colony, dx: i32, dy: i32| -> (u16, u16) {
        (0..14).flat_map(|r| (-r..=r).flat_map(move |a| (-r..=r).map(move |b| (a, b)))).map(|(a, b)| ((c.camp.0 as i32 + dx + a).max(1) as u16, (c.camp.1 as i32 + dy + b).max(1) as u16))
            .find(|&p| crate::colony::nav::passable(&c.map, p) && c.roof_over(p).is_none() && !c.marks.iter().any(|m| m.at == p) && !c.built_near(p)).unwrap_or(c.camp)
    };
    for (k, (sk, dx)) in [(StoneKind::Hall, -14), (StoneKind::Grove, -18), (StoneKind::Shrine, -22)].into_iter().enumerate() {
        let at = spot(&c, dx, 12 + k as i32);
        match c.place_stone(sk, at) { Ok(_) => shots.push((format!("{:?} stone", sk), (at.0 as f32 + 0.5, at.1 as f32 + 0.5), 2.5)), Err(e) => missing.push(format!("{:?} stone: {}", sk, e)) }
    }
    let marks: [(MarkKind, &str); 13] = [
        (MarkKind::Grave, "The grave of Aephre"), (MarkKind::Grave, "An old grave"), (MarkKind::Stone, "The stone of Thano"), (MarkKind::Stone, "The slab of Noostond"),
        (MarkKind::Stone, "The post of Boutrurn"), (MarkKind::Stone, "The bones of Gru the forgotten beast"), (MarkKind::Stone, "The refugees' fire"), (MarkKind::Stone, "The tribute stone"),
        (MarkKind::Scorch, "Scorched ground"), (MarkKind::Cage, "A cage trap"), (MarkKind::Cairn, "Gaunauth's cairn"), (MarkKind::Bench, "Thano's bench"), (MarkKind::Carving, "A carved post"),
    ];
    for (k, (mk, title)) in marks.iter().enumerate() {
        let at = spot(&c, -16 + (k as i32 % 7) * 4, 18 + (k as i32 / 7) * 4);
        c.marks.push(ColonyMark { at, kind: *mk, title: title.to_string(), text: String::new(), day: 1 });
        shots.push((format!("{:?}: {}", mk, title), (at.0 as f32 + 0.5, at.1 as f32 + 0.5), 2.2));
    }
    let at = spot(&c, 14, 20);
    c.marks.push(ColonyMark { at, kind: MarkKind::Cage, title: "The cage of a wolf".into(), text: String::new(), day: 1 });
    shots.push(("Cage with its catch".into(), (at.0 as f32 + 0.5, at.1 as f32 + 0.5), 2.2));

    // Every creature kind, and the night's things, in a row on open ground.
    let creatures: [(CreatureKind, &str, f32); 16] = [
        (CreatureKind::Raider, "a war band of The Git Clans", 1.0), (CreatureKind::Raider, "raiders of the Shadow of Saizsheik", 1.0), (CreatureKind::Raider, "a band of outlaws", 1.0),
        (CreatureKind::Besieger, "a war band of The Git Clans", 1.0), (CreatureKind::Trader, "traders of Ripu", 1.0), (CreatureKind::Wolf, "a wolf", 1.0),
        (CreatureKind::Wolf, "Aephre, risen", 1.0), (CreatureKind::Wolf, "Thano under the full moon", 1.4), (CreatureKind::Game, "red deer", 1.0), (CreatureKind::Game, "wild boar", 1.0),
        (CreatureKind::Game, "aurochs", 1.0), (CreatureKind::Pet, "ibex", 1.0), (CreatureKind::CaveHunter, "giant cave spiders", 1.0), (CreatureKind::CaveLife, "bats", 1.0),
        (CreatureKind::CaveLife, "pale crabs", 1.0), (CreatureKind::Beast, "Baelfang Storm-Caller", 2.4),
    ];
    for (k, (kind, name, size)) in creatures.iter().enumerate() {
        let at = spot(&c, 8 + (k as i32 % 6) * 7, -22 + (k as i32 / 6) * 8);
        let id = c.new_creature_id();
        c.creatures.push(Creature { kind: *kind, name: name.to_string(), pos: at, path: Vec::new(), stride: 0, leaving: false, size: *size, home: at, spawned: tick0, id, z: None, path3: Vec::new(), home_z: 0, out: false, rest_until: 0 });
        // (A flier is drawn above its shadow: crop round where it is seen.)
        let up = if *kind == CreatureKind::Beast { 2.0 } else { 0.0 };
        shots.push((format!("{:?}: {}", kind, name), (at.0 as f32 + 0.5, at.1 as f32 + 0.5 - up), if *size > 2.0 { 4.0 } else { 2.6 }));
    }
    // A snatched child returning among a band (the first raider carries them).
    if let (Some(child), Some(arc)) = (c.settlers.iter().position(|s| s.alive), c.arc.clone()) {
        c.snatched.push(crate::colony::snatch::Snatched { who: child, people: "The Git Clans".into(), faction: None, day: 1, seen: 0, home: None, raiders: arc.threat.clone(), coming: true });
    } else { missing.push("snatched child: no arc to come with".into()); }

    // Settler states: each shows on its figure or in its bubble.
    let alive: Vec<usize> = (0..c.settlers.len()).filter(|&i| c.settlers[i].alive).collect();
    let now = c.clock.tick;
    let day = c.clock.day();
    let mut states: Vec<(usize, String)> = Vec::new();
    let mut set = |k: usize, label: &str, c: &mut crate::colony::Colony, f: &dyn Fn(&mut crate::colony::Colony, usize)| { if let Some(&i) = alive.get(k) { f(c, i); states.push((i, label.to_string())); } };
    set(0, "a tantrum", &mut c, &|c, i| c.settlers[i].mind.broken = Some((crate::colony::mind::Break::Tantrum, now + 999)));
    set(1, "a fey mood", &mut c, &|c, i| c.mood = Some(crate::colony::mood::Mood { kind: crate::colony::mood::MoodKind::Fey, victim: None, who: i, since: now, days: 0, done: false, wants: "jade".into() }));
    set(2, "a broken arm, bandaged", &mut c, &|c, i| c.settlers[i].wounds.push(crate::colony::fight::Wound { part: "left arm".into(), severity: 3, healed_at: now + 99_999, from: "a raider's axe".into(), tended: 0, infected: false, fever_since: 0 }));
    set(3, "the lord", &mut c, &|c, i| c.settlers[i].office = Some("Lord of the camp".into()));
    set(4, "the priest", &mut c, &|c, i| c.settlers[i].office = Some("Keeps the temple of Ishra".into()));
    set(5, "a guest: a monster hunter", &mut c, &|c, i| { c.settlers[i].visitor = Some("a monster hunter of Titankeep".into()); c.settlers[i].guest_until = now + 99_999; });
    set(6, "in the stocks", &mut c, &|c, i| c.stocks = Some((i, now + 99_999)));
    set(7, "clothes in rags", &mut c, &|c, i| { c.clothes.insert(i, 1); });
    set(8, "the speaker", &mut c, &|c, i| c.settlers[i].office = Some("Speaks for the camp".into()));
    c.mandate = Some(crate::colony::society::Mandate::Watch);
    c.bell_until = now + 99_999;
    c.clash_at = Some(spot(&c, 4, 8));
    c.clash_tick = now.saturating_sub(120);
    if let Some(cl) = c.clash_at { shots.push(("the clash's aftermath".into(), (cl.0 as f32 + 0.5, cl.1 as f32 + 0.5), 3.0)); }
    let sg = spot(&c, -26, -6);
    c.siege = Some(crate::colony::siege::Siege { since: now, until: now + 99_999, who: "a war band of The Git Clans".into(), at: sg, sally: false });
    shots.push(("a siege camp".into(), (sg.0 as f32 + 0.5, sg.1 as f32 + 0.5), 4.0));
    let relic_at = spot(&c, 6, -8);
    if let Some(r) = c.relic.as_mut() { r.found = None; r.below = false; r.at = relic_at; let at = r.at; shots.push(("the lost relic".into(), (at.0 as f32 + 0.5, at.1 as f32 + 0.5), 2.0)); }
    else { missing.push("the lost relic: this camp has none".into()); }
    c.prisoner = Some(crate::colony::prisoners::Prisoner { name: "Gnaagh".into(), people: "The Git Clans".into(), faction: None, day, from: None, ransom_day: None });
    shots.push(("a prisoner at the post".into(), (c.camp.0 as f32 - 2.0, c.camp.1 as f32 + 2.0), 2.5));
    shots.push(("the mandate post, the bell, the fire, the store".into(), (c.camp.0 as f32 + 0.5, c.camp.1 as f32 - 0.5), 4.0));
    if let Some(&i) = alive.last().filter(|_| alive.len() > 9) {
        c.settlers[i].away_until = now + 99_999;
        c.expedition = Some(crate::colony::expedition::Expedition { beast: "Baelfang Storm-Caller".into(), party: vec![i], back: day + 9, tiles: 30, monster: None, harm: 0.0, striker: None });
        shots.push(("the signpost: an expedition away".into(), (c.camp.0 as f32 + 4.0, c.camp.1 as f32 + 2.0), 2.5));
    }

    // Each settler with a state stands on their own open ground (on day 1 they crowd the fire).
    for (k, (i, _)) in states.iter().enumerate() {
        let at = spot(&c, -30 + (k as i32 % 5) * 6, 26 + (k as i32 / 5) * 6);
        let s = &mut c.settlers[*i];
        s.pos = at; s.path.clear(); s.job = crate::colony::Job::Idle; s.z = c.map.surface_z[at.1 as usize * c.map.width + at.0 as usize];
    }
    if std::env::var("PLANET_DEBUG_INVENTORY").is_ok() { for (i, l) in &states { { let dp = c.draw_pos(*i); eprintln!("  {} -> {} office {:?} pos {:?} screen {:.0},{:.0}", l, c.settlers[*i].name, c.settlers[*i].office, c.settlers[*i].pos, (dp.0 + 0.5 - c.camp.0 as f32) * 20.0 + 1600.0, (dp.1 + 0.5 - c.camp.1 as f32 - 4.0) * 20.0 + 1300.0); } } }
    // Draw the camp as the window does, then crop each kind.
    let (fw, fh) = (3200usize, 2600usize);
    let cam = LocalCamera { cx: c.camp.0 as f32, cy: c.camp.1 as f32 + 4.0, tile_px: 20.0, z: 0, surface_view: true };
    let mut buf = vec![0u32; fw * fh];
    render_local(&c.map, atlas, &cam, &mut buf, fw, fh);
    super::local_ink::draw_colony(&c, &cam, &mut buf, fw, fh, history);
    super::viewer::save_rgb_png_pub(&format!("{prefix}_inventory_camp.png"), fw, fh, |x, y| { let q = buf[y * fw + x]; [(q >> 16) as u8, (q >> 8) as u8, q as u8] });
    // Settlers with their states, each cropped round their figure.
    for (i, label) in &states {
        if c.settlers[*i].away_until > now || c.roof_over(c.settlers[*i].pos).is_some() { missing.push(format!("{}: under a roof or away, not on the map", label)); continue; }
        let p = c.draw_pos(*i);
        shots.push((format!("settler: {}", label), (p.0 + 0.5, p.1 - 0.2), 1.3));
    }
    let (cw, ch) = (220usize, 200usize);
    let cols = 8usize;
    let rows = (shots.len() + cols - 1) / cols;
    let mut sheet = super::ink::Sheet::new(cols, rows, cw, ch);
    sheet.title("Every kind, in the game's own frame (cropped from the test camp)");
    let n = shots.len();
    for (label, (x, y), half) in &shots {
        let (cx, cy) = sheet.cell(&label.chars().take(34).collect::<String>());
        // Crop: the cell window round (x, y), scaled to fit 200 x 160.
        let (sx, sy) = ((x - cam.cx) * cam.tile_px + fw as f32 / 2.0, (y - cam.cy) * cam.tile_px + fh as f32 / 2.0);
        let span = half * 2.0 * cam.tile_px;
        let scale = (180.0 / span).min(3.0);
        let (ow, oh) = (span * scale, span * scale * 0.8);
        let mut put = sheet.put();
        for oy in 0..oh as i64 { for ox in 0..ow as i64 {
            let (px, py) = ((sx - span / 2.0 + ox as f32 / scale) as i64, (sy - span * 0.4 + oy as f32 / scale) as i64);
            if px < 0 || py < 0 || px as usize >= fw || py as usize >= fh { continue; }
            let q = buf[py as usize * fw + px as usize];
            put((cx - ow / 2.0) as i64 + ox, (cy - oh / 2.0) as i64 + oy, [((q >> 16) & 255) as f32, ((q >> 8) & 255) as f32, (q & 255) as f32], 1.0);
        } }
    }
    sheet.save(&format!("{prefix}_inventory.png"))?;
    Ok((n, missing))
}

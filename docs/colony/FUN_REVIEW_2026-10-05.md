# Dark Tower: is it fun yet?

Design review, 2026-10-05. External consultant's report: what will make the game more fun to
play, ranked, with evidence. No game code was changed. The 35 cards that follow from it are on
the Dark Tower Board, titled as listed in section 6 (card ids start with `fun-`). The owner
answered the three open questions the same day; the answers are in section 7 and applied
throughout. Sections 8 and 9 were added the same day at the owner's request: smarter settlers
and more elaborate building, then monsters, depth and reasons to dig.

**What was played.** A snapshot of the release binary built from the working tree at 11:32 on
2026-10-05 (HEAD `f0ecbc9` on `next-cards`, plus the uncommitted colony-projects work). Another
session was editing `src/colony/mod.rs` and `src/tiles/viewer.rs` during the review, so line
numbers below may be off by a few. No window can be opened from this environment, so
everything was run through the headless flags: about 25 worlds (dev size, 256x128, one
512x256, six world styles) and about 50 colony runs. Four code audits ran in parallel
(gameplay, procedural generation, rendering and UI, performance). What that leaves unverified
is in section 7.

---

## 1. Brief

- **Pitch and genre.** An autonomous, story-first colony simulator in the line of Dwarf
  Fortress and RimWorld, set on a procedurally generated planet with 250 years of simulated
  history and a spreading dark power, the Shadow. The player is a *patron*, not a foreman:
  the settlers run their own lives, and the player watches and nudges with a small budget of
  favour (`ROADMAP.md`, pillars 1-7). The claimed edge over the genre: every event can answer
  "why?", the world keeps living around the colony, and a run is deterministic, so it can be
  shared as a code and replayed.
- **Target player, platform, session.** The docs never state these outright. Inferred: players
  of DF and RimWorld who enjoy reading and retelling emergent stories more than optimising a
  base. Desktop only (Rust, a software-rendered `minifb` window at 1280x800, built on macOS,
  CI on Ubuntu). A session is "an evening", and a full run is "20 years" of colony time
  (`ROADMAP.md`, Update 6 exit).
- **Intended experience.** Wonder at a world that has a past; attachment to settlers who carry
  that past; dread of the Shadow; the sense of having nudged the story; and the urge to post
  the map and retell what happened (the owner's own measure of success).
- **Current state.** v0.1, about 83,000 lines. The planet, the history and the lore are
  mature. The viewer draws the world in a distinctive ink-cartography style. The colony is a
  vertical slice: seven settlers with pasts, a hut, four self-chosen projects, one story arc
  (rumour, refugees, raid), four patron verbs, three founding stones, marks and graves, a
  saga page. All six roadmap updates are formally open; the slice pulled part of Update 3
  ahead of Updates 1 and 2.
- **The board.** The "Dark Tower Board" artifact
  (<https://claude.ai/artifact/RnaddpSsNUzVViX3sZMFhn>), collection `cards`. A card is
  `{column, title, details, status: todo|doing|done, origin: claude|you, order, created}`.
  Nine columns: terrain, history, shadow, lore, viewer, graphics, embark, colony, tooling.
  `details` is free text; review cards carry a priority (P0-P2), the problem with its
  evidence, and a "Works when:" line. Before this review: 123 cards, 73 done, 50 to do.

---

## 2. Verdict

**The world is fun. The colony, which is the game, is not fun yet.**

What exists is a superb toy and a five-minute diorama. The toy is the world: the ink map, the
watcher, and above all the present-day card are the best hook in the project ("What can wound
it: The Hammer of Braifnelm, in the hoard of Ithiel the Eternal"). That delivers discovery,
one of the five feelings the game is after.

The diorama is the colony. At the slowest speed, everything authored for it is over in 5
minutes 8 seconds: a rumour at 52 s, refugees at 130 s, the raid at 308 s. After that nothing
happens, through day 365. In that time:

- **The player decides nothing that matters.** Fifteen runs (five seeds, each with no patron,
  a careful patron and a deliberately bad one) ended with the same survivors, the same
  buildings on the same days and the same raid sentence. The raid is a seeded dice roll.
- **There is nothing to watch.** A game day lasts 24 seconds at the slowest speed, and a
  settler crosses the whole map in a fifth of a second. Settlers are dots that blink between
  places.
- **The story is off screen.** The clock, the favour count, the settler's "why" and the
  result of each patron act share one line in the OS title bar. The log is never drawn in the
  window. A death is a line of text the player cannot see.
- **Every colony is the same colony.** Nine seeds gave the same events on the same days; only
  the names changed.

The fun is leaking at the joins. The world's loaded present (a war, twelve grudges, nine
beasts, the Shadow's frontier, its bane in a dragon's hoard) reaches the colony as one name in
one sentence. The settlers' pasts, the best raw material for attachment, are captions that
change nothing they do.

**The single most important change:** make the raid something the patron can win or lose, and
show it. Let what the colony built and what the patron chose decide who lives, print that sum
on screen before the night comes, and give the night a moment: pause, camera, a card with its
"because". Three small cards (`fun-raid-agency`, `fun-colony-hud`, `fun-event-moments`) turn
the existing five minutes from a screensaver into a first decision. Everything else in this
report builds outward from that.

---

## 3. What's working (keep and protect)

- **The ink world map.** Washes, sepia outlines, IM Fell lettering, the Shadow's hatched
  dominion and black tower. It reads as one hand and looks like nothing else in the genre. The
  poster is the most shareable thing the game makes today.
- **The present-day card.** "The world today" at the end of the watcher is a pitch for a game
  in six lines: who is at war, what the Shadow is pressing on, what can wound it and where
  that is. Protect the loaded present behind it (`history/present.rs`,
  `tests/present_day.rs`).
- **Receipts.** 75% of events carry a cause, and the inspector's "Because / It led to" chains
  are the best reading in the game.
- **Determinism and the dev world.** A world with 250 years of history in 0.3 s, a colony
  month in 0.2 s, byte-identical reruns, world codes, a headless flag for every view. This
  iteration loop is why the project moves fast. Every recommendation below can be judged with
  it.
- **Autonomy that holds.** On a workable site the settlers feed, house and defend themselves
  for 365 days with nobody stuck. The "why" lines are honest: they come straight from the
  utility scores.
- **Pasts, marks and epitaphs.** Settlers cite real battles and real fallen towns; a grave
  carries a line from the life it ended. This is the right material. It needs a stage.
- **The no-orders patron.** Favour instead of commands is the game's most distinctive idea.
  Keep the constraint. It needs teeth.
- **The watcher's structure.** Playback paced by drama, a timeline to scrub, a closing card.
  The colony should borrow all three.

---

## 4. Top issues, ranked by impact on fun

### 4.1 The patron cannot change anything

**Problem.** The raid is the only event with stakes, and nothing the player does moves it.

**Evidence.**
- `src/colony/arc.rs:206`: `ready = 0.05 x nights of watch + 0.25 if the hut stands + 0.03 x
  settlers`. The watch is automatic (one settler a night from day 6), the hut is always up by
  day 2, the refugees always make nine. Readiness is 0.92 in every healthy colony: the raid
  line says "8 nights of watch kept, the hut standing, 9 to fight" in 20 of 20 surviving
  colonies, across forest, desert, tundra, jungle and town sites.
- `arc.rs:210-211`: `danger = strength + 0.6 x a hash of the seed`. A death needs danger above
  readiness + 0.3. So the odds are fixed by the threat's kind (death / rescue / rout): beast
  30/50/20, the Shadow 13/50/37, a war band 0/47/53, outlaws 0/13/87.
- Five seeds (76, 58, 23, 5, 3), each run three ways: no patron, a best-effort patron (15
  acts), a deliberately bad patron (20 acts: the hall stone 55 cells from camp, four grove
  stones round the camp, a dream of rest every dawn). All 15 runs: same survivors, same raid
  sentence, same builds on the same days. Hut completion moved by at most 5 game hours, which
  is 5 real seconds. The bad patron ended with more food on 3 of 4 seeds. On seed 58 a
  settler dies by a margin of 0.005 and no input can save him.
- The verbs, as built:

| Verb | Cost | What it does | Verdict |
|---|---|---|---|
| Bless ground (F) | 1 favour | x1.4 on forage and felling in a 13x13 square, only while the camp is short | No effect on any outcome |
| Forbid ground (X) | 1 favour | Blocks 0.46% of the map; cannot be lifted | No effect |
| Favour a settler (G) | 1 favour | x1.15 on their work; "the others notice" is text only | No effect |
| Dream (D) | 1 favour | The game picks the dream (`viewer.rs:466-467`); from about day 3 it is always Rest | Mildly harmful |
| Hall stone (H) | Free | Only before the first log is laid, which is 3-5 real seconds after an unpaused start | Cosmetic |
| Grove, shrine (J, K) | Free | Keeps trees in a radius of 7; half the settlers rest by the shrine | Cosmetic |

**Why it hurts.** Favour is never tight, there is nothing worth spending it on, and the best
strategy is to do nothing. Pillar 6 ("influence is a limited budget") has no budget pressure
behind it. A player who works this out, and they will inside one session, has no reason to
touch the game again.

**Direction.** Put unattended readiness mid-range (about 0.6) and give the patron about 0.3 of
swing, using things already in the game: count the palisade (+0.15 when closed), make the
colony keep watch only every other night unless a dream or a blessing at the camp's edge adds
a watcher, let a favoured veteran captain the watch (+0.1). Print the sum before the night
comes ("The watch: 5 nights. The palisade: half raised. Nine to fight. The trader says it is
not enough."). Target: over the six dev seeds a careful patron changes the outcome on at
least four, and a careless one loses someone on at least three.

**Alternative.** Keep the roll and let the patron spend all three favour on the night as a
ward. It is a day's work, but it is a button, not a story, and it teaches the player to hoard.

### 4.2 Nothing to watch: settlers teleport

**Problem.** Walking is free. A settler covers 15 cells in a game minute, so at every speed
people blink from place to place.

**Evidence.**
- `src/tiles/viewer.rs:407`: 60 ticks a second at 1x (a tick is a game minute). A day lasts 24
  s at 1x, 8 s at 3x, 2.4 s at 10x.
- `src/colony/mod.rs:802-803`: a settler walks up to 15 cells per tick, so 900 cells a second
  at 1x. The embark is 192 cells wide: crossed in 0.2 s. Positions are drawn as integer cells
  with no tween (`src/tiles/local_ink.rs:588`).
- Crossing the map costs 13 game minutes, so distance never matters. A hut forced into the far
  corner by a hall stone finished 4.5 hours *sooner* than the default: noise.

**Why it hurts.** The pitch is "built to be watched", and the roadmap promises a follow-cam on
a settler. At every speed the colony is dots blinking between places; the player cannot follow
one person through one morning. It also guts the spatial verbs: blessing, forbidding and
placing stones cannot matter where distance is free.

**Direction (decided 2026-10-05: like Dwarf Fortress).** Dwarf Fortress has a calendar as
brisk as this one (1,200 ticks a day, about 12 real seconds) and is still a game about
watching individuals, because walking is decoupled from the calendar: a dwarf takes about ten
ticks a tile, so it needs about a day and a half to cross a 192-tile map. Do the same here. A
starting point to tune by eye:
- Keep the calendar: a day stays 24 s at the normal speed.
- Walk about 1 cell every 5 ticks instead of 15 cells a tick: 12 cells a second on screen,
  the map crossed in about 16 game hours. Draw settlers between cells.
- A settler then does a handful of jobs a day, as a dwarf does. Rescale work and appetite to
  that, so the hut takes days and the first fortnight is full.
- Keep the faster speeds for the long game and add Skip, to the next log line or to dawn.
  Nine settlers simulate at about 1,200x, so a skip is instant.

This is the largest retune in the report, and it pays twice. Distance starts to cost time, so
where the hut stands and which ground is blessed begin to matter. And "every project is one
day's work" ends by itself.

**Not chosen.** Keeping realistic walking and telling the colony as a chronicle with event
cards (a Crusader Kings pace).

### 4.3 The story happens off screen

**Problem.** The colony has no interface. Its best content is written to files only the
developer reads.

**Evidence.**
- The whole HUD is the OS window title (`viewer.rs:548-552`): clock, world code, favour, the
  hovered settler's "why", the key list and, last, the result of the player's own act. About
  270 characters in a 1280 px title bar.
- `colony.log` is never drawn in the window; its only readers are the headless paths
  (`viewer.rs:1188`, `:1499`). My 30-day run produced 26 log lines and 3,614 reasoned
  decisions. The player sees none of them.
- Only the three arc beats raise a banner, and the banner shows a title ("The raid") with no
  text (`src/colony/arc.rs:142`, `local_ink.rs:619-630`). It lasts 720 ticks: 12 s at 1x, 1.2 s
  at 10x. No pause, no camera move. A death by hunger, a finished hut and "falls ill" get a
  log line nobody can see.
- Favouring a settler or sending a dream leaves no mark on the map.
- There is no audio anywhere: no audio crate in `Cargo.toml`, no audio code in `src/`.
- The tale, the saga page and the decision trace exist only behind `--sim-snapshot`.

**Why it hurts.** Pillar 4 says legibility is a feature. Today the player presses G and
nothing visibly happens; the arc's climax can be missed by blinking; and the one thing this
game does better than its rivals, saying *why*, is invisible in play.

**Direction.** Two small pieces, both drawn after `draw_colony` (`viewer.rs:828`) with the
existing `ui::card`:
- A HUD: a chip with day, hour, speed and three gold favour pips; a log card with the last six
  lines; a hover chip like the watcher's (`watcher.rs:947-961`); a strip that names the verbs
  and their keys.
- A moment for every event that matters (arc beats, deaths, a finished building): pause, ease
  the camera there over 600 ms, show a card with the text and its "because" until Space.

Sound is out of v0.x (decided 2026-10-05), so the pause, the camera and the card have to
carry every moment by themselves.

### 4.4 No pressure, and no second act

**Problem.** Nothing is scarce, nothing escalates, and nothing ends.

**Evidence.**
- Timeline at 1x: hut at about 30 s, four projects between 48 s and 156 s (woodpile, palisade,
  drying rack, second hut, each logged as "1 days' work"), raid at 308 s. Then nothing: my log
  has no line between day 14 and day 30, and `--sim-projects 365` ends with the same four
  projects and 9 of 9 alive. After the raid the arc falls through (`arc.rs:193-194`).
- Of 3,614 decisions in 30 days, 1,066 (29%) are "Nothing needs doing; resting near the
  fire"; 83% are wander, eat or sleep.
- Food never fell below 20 meals on the default site. Cold cannot bite: a nine-hour night adds
  at most 0.90 exposure and illness needs 0.98 (`mod.rs:499-504`), so "falls ill" never fires.
- The projects are scenery. The palisade ("for fear of...") is not in the readiness sum. The
  drying rack ("would spoil") has no spoilage behind it. The woodpile only stamps walls
  (`src/colony/projects.rs:136-141`).
- No winter (embarks have no seasons), no farming, no way to win or lose, no ending. Esc
  discards the colony (`viewer.rs:419-430`; nothing in `src/colony/` is saved).

**Docs versus game.** `ROADMAP.md` Update 3 exits on "three years with no input: fields
planted, houses built, a winter endured". The slice has no fields and no winter and has been
judged over 30-60 days. `src/colony/CLAUDE.md` says "Exposure matters"; it cannot.

**Why it hurts.** Tension needs something that can run out and a clock that says when.
"One more day" needs a next thing to wait for. The game has neither after minute five.

**Direction.** Give each project one number (palisade to readiness, woodpile to exposure at the
fire, rack to a six-day food life), lower illness to 0.85, and stop shrubs bearing in winter.
After the raid, plan the next trouble from the next open thread in `present()`. Let a colony
end (the last settler dies; the first winter is survived) and write its saga when it does.

### 4.5 The loaded present never reaches the camp

**Problem.** The world, the people and the colony are three layers that barely touch.

**Evidence.**
- The dev world's present day holds 1 war, 12 grudges, 9 living beasts, 7 towns on the
  Shadow's frontier, the Shadow's bane in a beast's hoard and 10 towns fallen in 25 years. The
  colony uses one threat's name and one fallen town's name. `present()` is not read by the
  colony at all.
- A settler's past is read only by the epitaph, the inspector, the saga and the portrait
  (`mod.rs:376-378`). Behaviour differs between settlers only by five random taste numbers.
  Settlers never interact with each other.
- The roster is a recipe: three survivors who "hate X, who took Y", two veterans with two
  identical lines ("Fought at the Battle of Hallowedkeep in 444 under Sloarumth. Came home
  when the War of Brolmdustoor's Independence ended in 445."), two kin who "miss Z". All seven
  come from one town.
- Choosing where to settle is blind. The site line is a comma list in the title bar ("water,
  wood (24 trees), stone, berries...", `src/local/site.rs:166-186`). It says nothing about
  towns, peoples, threats or the Shadow. `ROADMAP.md` Update 1 promises a site panel that
  "tells you what is true now" and a choice of people; neither exists.

**Why it hurts.** "The past you inherit" is the game's reason to exist. Today the past is a
caption under a dot. A veteran and a child of a burned town do the same things for the same
reasons.

**Direction.** Three hooks, each a few lines: veterans take the watch and add readiness;
"hates X" fires when X is the threat or when a refugee is of X; shared watches and meals feed
a small opinion table. Offer three sites at the end of the watcher, each with one line of what
is true there now. Draw the second and third arcs from `present()`: the war on the border, the
grudge, the rumour of the bane.

### 4.6 Every colony is the same script, and a site is either "the same" or "dead"

**Problem.** The variety stops at the colony's edge.

**Evidence.**
- Nine seeds at the default site: the same event sequence in 9 of 9. First berries 06:46, hut
  day 2, woodpile day 3, rumour day 3 10:00, palisade day 4, drying rack day 5, refugees day 6
  16:00, second hut day 7, raid day 14 02:00. The beat days are constants (`arc.rs:55-57`).
  The day-30 camp is the same picture in forest and in desert: ring palisade, two huts,
  woodpile, scorch mark, in the same places.
- `furnish` (`src/local/site.rs:102-140`) tops every site up with a spring, a 24-tree grove, a
  20-cell outcrop and berries, so the grove appears as a clump in open desert and in a tundra
  town. "Old bones" is in 11 of 11 site reports.
- Four of 15 land sites on seed 76 kill everyone by day 10, in silence. The coast at 12,34 has
  the richest report of all (2,329 trees, 494 berry bushes, a river) and logs "0 food, hut no
  site"; seven die on day 6, the refugees "stumble into camp and are taken in" by graves, and
  starve by day 10. The mountain (48,18) and crater (28,38) colonies finish most of a hut while
  starving; the ruin (61,19) is the fourth. These ran through the headless path; the window
  refuses only water (`viewer.rs:618`).

**Why it hurts.** The base and its first story are what players would post and retell, and
they are the same picture and the same paragraph for everyone. And the only failure the game
has is unfair: no warning before, no explanation after.

**Direction.** Let the beats slip by seed and state (rumour day 2-5, refugees 5-9, raid 11-18)
and make the refugees a choice. Give each site an identity instead of a top-up: one scarcity,
one gift, one mark from history, said honestly before the player commits. A site that cannot
feed seven is refused or labelled hard, and a colony that dies says why.

### 4.7 The first minute loses people

**Problem.** A new player meets eleven generator settings, a vanished window, an ocean, and
two keys nobody told them about.

**Evidence.** The path from `cargo run --release`:
1. The start screen: eleven rows about the generator. "Watch it unfold" defaults to No
   (`src/main.rs:672`), so the default path skips the game's best opening and the present-day
   card.
2. The estimate reads "History: about 6 min" (`src/tiles/start.rs:62-70`). Measured: 10.9 s
   for the whole Standard world with 250 years.
3. The window closes. For those 11 s (66 s on Large) progress goes to the terminal.
4. A new window opens, titled "Planet viewer", at the map's centre at 16 px a tile
   (`viewer.rs:284-294`). On seed 42 that is mostly open ocean.
5. To play, the player must press Z over a tile, then Enter. Both keys appear only in the
   title bar.
6. Z freezes the window for 11-15 s while the region is simulated on the UI thread
   (`viewer.rs:754-775`), then shows a satellite-style relief map in a different art style.
7. Enter founds the colony, unpaused, with no goal stated anywhere.

**Why it hurts.** The hook exists (the watcher and its closing card) and the default path
routes around it. Nothing says what the game is or what to do next.

**Direction.** Watch on by default; an honest estimate; open the map fitted on the Shadow's
frontier; keep one window open from Begin to the camp with a parchment progress card; end the
watcher with three sites to click instead of "choose where to settle".

### 4.8 The world's story has one shape too

**Problem.** Geography varies in kind. History varies in names and numbers.

**Evidence.**
- The Shadow is four acts on a timer. Over 18 worlds: it rises in year 201 every time, "The
  Last Alliance storms X", the alliance always wins, and the Return is the Check plus exactly
  20 years (`src/history/shadow.rs:35`, `:412-413`). Seed 76 gives 358 and 378 under all six
  world styles.
- The villain shrinks as the map grows: it holds 3-22% of the land on dev worlds, 2-4% at
  256x128 and 0% (5% reach) on the default 512x256 world, where the Dark Tower is a speck.
- The world's one sentence is a long reign in 15 of 19 worlds. On seed 76 it is false:
  "Eaiindtaer ruled The Republic of Sildor for 188 years, and rules still", but Sildor was
  destroyed in 444 (`src/lore/claims.rs:54-55` checks only that the ruler is alive).
- "Rare" is not rare at size: "Unbroken peace (1 world in 4)" appears in 6 of 6 worlds at
  256x128 (`src/lore/rare.rs` rates are measured on dev worlds only).
- Seed 76 has 449 battles and 85 battle names; "The Battle of the Brolmdustoor Pass" was
  fought 44 times. Wars run to "The Sixth Vengeance of Suncoil". "The Titancairn Ocean" is on
  7 of 12 dev maps.

**Why it hurts.** A player on their third world recognises the plot. It matters less than 4.1
to 4.6 because nobody reaches a third world yet.

**Direction.** Later, not now (`fun-shadow-shapes`): four to six authored outcomes of the
Check chosen by the state of the world, a reach that scales with the map, and a headline that
is fact-checked against the present.

### 4.9 Traps and broken promises

- **D pans the camera and sends a dream** (`viewer.rs:391` and `:465`). Panning right with a
  settler under the mouse spends a favour.
- **Space resumes at 1x**, losing 3x or 10x (`viewer.rs:402`).
- **Esc discards the colony without asking.** One more Esc is the map; the next quits.
- **Marks and stones cannot be removed.** Forbidden ground is forbidden for good.
- **The colony cannot be named in the window.** `name_colony` is called only by trials and
  scripts, so every saga is "The Saga of The camp at 45,12".
- **The world code in the title does not reproduce a walked-in colony.** The window seeds it
  from cell coordinates (`viewer.rs:622`), `--code` from the tile (`viewer.rs:319`). From
  reading the code; not run. `--interventions` is read only by `--sim-snapshot`
  (`src/main.rs:1855-1858`), so "one seed, many fates" is a developer tool.
- **Receipts that contradict each other.** In one party, founders say Brolmdustoor "fell in
  411" and the refugees say "fallen in 392". A beast whose lair is "34 days' walk from here"
  raids 11 days after the rumour. Gru, 22, "left Glamourshine after the siege of Glamourshine
  (276)", 175 years ago. And the world's headline sentence on seed 76 is false (4.8).

A game that sells receipts loses the sale the first time a receipt is wrong.

### 4.10 Two more art styles, and a camp that cannot be read

- **The 8x8 bitmap font carries every panel:** the start screen, the whole watcher, the
  inspector, the settlers' name tags. The present-day card, the game's pitch, is set in it at
  8 px. IM Fell is already loaded and the poster's cartouche shows how to use it
  (`viewer.rs:1012-1032`).
- **The region view** (every player crosses it to embark) is saturated hillshade with a red
  marker and a yellow box. A card for this exists ("Zoom view shows tiles, not shaded relief").
- **A settler is a flat disc about 10 px across.** Seven coat colours serve nine settlers.
  Nothing shows what anyone is doing. Stumps are the same size as people and outnumber them.
  Sleepers are drawn on top of the roofs.
- **There is no night.** Day and night frames are identical.
- **Black bands** above and below the map near the poles; the viewer lacks the watcher's
  `clamp_cy`, and the band leaks into plates.
- **The saga page** runs its last entry off the canvas, cuts cast lines mid-word, and spends
  six of its nine timeline entries on day-1 and day-2 firsts (first berries, first fish, first
  tree).

### 4.11 Performance the player feels

Measured on an M4 Pro (14 cores) while other work was running; render numbers are 10-30% high.

| Moment | Measured | What the player gets |
|---|---|---|
| After "Begin", Standard world | 10.9 s (world 3.5 s, history 7.4 s) | No window at all, under an estimate of "6 min" |
| After "Begin", Large world | 65.6 s, 1.34 GB | The same; 17 s of it is one quadratic step (`calculate_river_crossings`, `src/region/rivers.rs:167`) |
| First Z on a new area | 11-15 s | A frozen window with every core busy (`viewer.rs:754-775`) |
| Walking near a town | 0.7-1.5 s per step of the site line | Stutter; `generate_local` runs on the UI thread every 24 cells (`viewer.rs:658-663`) |
| Three game years at the 10x cap | 44 min | Nine settlers could run at about 1,200x |
| Colony view while the clock runs | Full-map redraw every frame (`local_ink.rs:109-136`) | Estimated 6-8 busy cores; not measured in a window |
| 500 settlers | About 200 ticks a second (2,958 at 200) | A cliff after 200; `decide` rescans the map and hashes with SipHash |

Fast enough already, leave alone: the dev world (0.3 s), save and load (0.05 s), the journal,
plates, the poster, map frames (11-22 ms), the colony tick up to 50 settlers.

### 4.12 Scope: where the effort goes

- By lines: world generation 33%, history 26%, viewer 11%, exporters 7%, frozen terminal UI
  5%, the colony 2% (1,812 of 83,228 lines).
- 114 command-line flags. The saga, the tale, the poster, the arms sheet, replaying
  interventions and naming the colony are reachable only through them.
- Built ahead of the loop they serve: the saga page, portraits and the plate legend (there is
  no story to put on them yet); z-levels, strata and ore (the colony never digs); wildlife
  signs (nobody hunts); the province snapshot.

---

## 5. Design direction

**Pillars to commit to.**

1. **The patron's hand decides the night.** A few weighty, visible interventions. Each has a
   cost the player feels and a consequence the game prints. Never orders.
2. **Watched like Dwarf Fortress, skippable at story speed.** At the normal speed settlers
   visibly walk and a day passes in seconds. A director pauses, points the camera and shows a
   card when something worth seeing happens. A keypress skips a quiet week.
3. **The past walks into camp.** Every beat in the colony is drawn from the world's present,
   and a settler's past changes what they do, not only what their page says.
4. **Pressure makes story.** Something can run out, winter comes, the next trouble is on its
   way and was foretold. Every loss has a "because" and leaves a mark.
5. **Every run ends on a page worth posting.** Endings, a name, a saga composed by the game.
   This one comes last: it needs the first four to have something to show.

**Stop doing, until a stranger can win or lose the first raid:**

- More terrain, climate or biome realism.
- New sharing artifacts and polish on the existing ones (saga, portraits, poster variants).
- Bard and director features.
- Architecture styles. The underground and its monsters wait for Phase 4 (section 9).
- New features that are reachable only by a command-line flag. If the player needs it, it
  gets a key or a button.
- Judging Update 5 work on the dev world: its tiles are 417 km across and its nearest town is
  43-75 days' walk away.

Out of v0.x altogether, by the owner's decision: sound.

---

## 6. Roadmap

### Phase 1: quick wins

*Goal: the five minutes that exist are legible, honest and free of traps.*

| Card | Column | Priority | Effort |
|---|---|---|---|
| The raid must be the patron's to win or lose | colony | P0 | S |
| The colony's HUD and log, in the window | graphics | P0 | M |
| Every colony event gets its moment | viewer | P0 | S |
| Keys that betray the patron | viewer | P0 | S |
| Every project earns its sentence | colony | P0 | S |
| No silent wipes: a doomed site says so | embark | P0 | S |
| First minute: watch by default, honest estimate, open on the frontier | viewer | P0 | S |
| Receipts that contradict each other | lore | P1 | S |

### Phase 2: next milestone, "The Night of the Raid"

*Goal: a stranger plays a 20-30 minute first session, can save or lose a settler, or the whole
colony, by their own choices, and wants a second one.*

| Card | Column | Priority | Effort |
|---|---|---|---|
| A clock you can watch, as in Dwarf Fortress | viewer | P0 | L |
| Settlers save themselves before they build | colony | P0 | M |
| Creatures on the map: the raid arrives on legs | colony | P0 | M |
| Settlers who plan ahead: the camp's worries, said aloud | colony | P1 | M |
| Read the camp at a glance: figures, jobs, night | graphics | P1 | M |
| The refugees are a choice, and the beats keep no timetable | colony | P1 | S |
| The past walks into camp: pasts that act | colony | P1 | M |
| After the raid: the next trouble, and winter | colony | P1 | M |
| A colony can end, be left, and be found again | colony | P1 | M |
| Choose where to settle: three sites and what is true there | colony | P1 | M |
| One window from Begin to the camp | viewer | P1 | M |
| One hand for the words: IM Fell on the cards that carry the story | graphics | P2 | M |

### Phase 3: longer term, "A village of their own making"

*Goal: the second colony is a different story in a different village.*

| Card | Column | Priority | Effort |
|---|---|---|---|
| Skill makes roles: a builder, a hunter, a keeper of the fire | colony | P1 | M |
| Buildings with a job: store, smokehouse, well, tower, workshop, field | colony | P1 | M |
| A village of their own making: a plan that follows the ground | colony | P1 | L |
| Buildings rise in stages and grow: tent, hut, house, hall | graphics | P2 | M |
| Each people builds its own way: dug halls, tree houses, war camps | embark | P2 | L |
| The Shadow's story has more than one shape | shadow | P2 | L |

Also in this phase, already on the board: the province pass (neighbours within two weeks'
walk), the bane as a quest in play, Update 4's relationships, and the rewind-and-fork screen.
Raiders that walk onto the map moved up to Phase 2 ("Creatures on the map").

### Phase 4: "Under the hill"

*Goal: a colony digs for a reason it can state, finds something history left there, and
something finds it.*

| Card | Column | Priority | Effort |
|---|---|---|---|
| Mountains at human scale: a rock face to dig into, and a living to be had | embark | P0 | M |
| Settlers go under: the pick, the stair, and walking in three dimensions | colony | P0 | L |
| Reasons to dig: warm, safe, cool, rich | colony | P0 | M |
| Seeing below: the cutaway and the section, in ink | graphics | P0 | M |
| Something down there: caves, lairs, old works and sealed doors | embark | P1 | L |
| The unknown is blank, and someone has to go in | colony | P1 | M |
| A small bestiary with causes: who they are, why here, what they want | history | P1 | M |
| Dig too deep: every level is richer and nearer to what sleeps | colony | P1 | M |

Phase 4 needs only two cards from the village work: "Settlers who plan ahead" and "Buildings
with a job". If monsters and depth matter more to the owner than village layout, it can go
before the rest of Phase 3.

---

## 7. Open questions and assumptions

**Decided by the owner, 2026-10-05.**

1. **Watch people, or read a chronicle? Like Dwarf Fortress.** The player watches individuals.
   Section 4.2 and the clock card follow Dwarf Fortress's model: a brisk calendar, with
   walking slow enough in game time to follow on screen. That reading of the answer is mine.
   The "Read the camp at a glance" card stays.
2. **Can the first colony fail in its first session? Yes.** The raid, hunger and cold may
   kill, up to the whole colony, provided the failure was foretold and is explained (4.6).
   The ending in "A colony can end, be left, and be found again" is part of the Night of the
   Raid milestone, not an extra.
3. **Does sound belong in v0.x? No.** The "Five sounds for five moments" card was removed from
   the board. Moments are carried by the pause, the camera and the card.

**Assumptions.**

- Target player and session length are inferred, not documented (section 1).
- The dev world remains the place to judge colony work, but not anything that depends on
  neighbours (section 5).
- The suggested numbers (readiness 0.6, walking 1 cell every 5 ticks, a 24 s day) are starting
  points to tune with the headless trials, not targets.

**What I could not run or verify.**

- Anything in a real window: input latency, frame pacing, how much of the title string macOS
  shows, the feel of banners, the start screen and watcher in motion. The key clashes, the Esc
  behaviour and the world-code mismatch are from reading the code.
- `cargo build` and `cargo test`: another session was building the tree, so I ran a copy of
  its binary instead.
- CPU use of the colony view while watching (estimated from single-thread frame times).
- More than one 512x256 world; the bard and the director (they need a local model).
- Four land sites and one lake site wiped through `--sim-snapshot --tiles-center`; I did not
  confirm the window accepts those sites.

---

## 8. Addendum: smarter settlers and a village worth building

Added 2026-10-05 after the owner asked for settlers that are more intelligent and build more
elaborate structures. It is the right wish: in a game where the player gives no orders, how
the settlers think *is* the gameplay and what they build *is* the base. It also answers 4.4 to
4.6 (no second act, one script, sites that play alike). Seven cards follow from it.

**How settlers think today.**
- Each settler picks the best of ten options (eat, sleep, forage, fish, fell, quarry, haul,
  build, wander, keep watch) by what is true now: hunger now, 4 meals a head stored now, logs
  wanted now (`src/colony/mod.rs`, `decide`). Nothing looks ahead.
- The camp's plan is a fixed ladder of five works, taken one at a time and each built once:
  palisade, second hut, windbreak, woodpile, drying rack. Then it returns nothing, for good
  (`src/colony/projects.rs`, `plan_projects`).
- Settlers differ by five random "taste" numbers. Nobody has a skill, nobody improves, nobody
  has a role.
- The worst moments are bad: one colony starves beside 494 berry bushes; two lay 30 and 34 of
  a hut's 36 logs while all seven starve; nobody searches past 60 cells or moves the camp. On
  a good site all nine rest by the fire from day 15.

**How they build today.**
- A colony's whole output is two huts, a palisade, a woodpile and a drying rack. Three of the
  five are a 3x1 block, a line and four posts.
- Each goes on the nearest flat footprint within 10 cells of the fire (`find_site`). The
  palisade is a circle of radius 11 whatever the ground. The camp is the same picture in
  forest and in desert.
- A building appears whole when its last load is laid. Nothing is extended or repaired.
- The world's own towns are laid out by a real generator (`src/local/structures.rs`,
  `plan_for`: streets, lots, a plaza, a wall, fields), and `ROADMAP.md` Update 3 says that
  generator "becomes the plan they grow toward". The colony uses none of it. It also uses
  none of the crops, the game trails or the z-levels the embark already has.

**Advice.**

1. **Remove stupidity before adding cleverness.** Players judge a mind by its worst moment.
   Until nobody starves beside food or builds while starving, nothing else reads as
   intelligence.
2. **Foresight, said aloud, is what reads as smart.** "Winter is 40 days off and the store
   holds 9 days of food: they begin a smokehouse" needs a ranked list of worries, not a
   planner. Hold GOAP, HTN and personal ambitions (Update 2) until that list has run out of
   road.
3. **A building is elaborate when it is specific, legible, seen rising and partly yours.**
   Specific to this ground; there for a reason the player can read; visible going up; shaped
   by the patron. More cells and more kinds do not get there on their own.
4. **Blessing and taboo become town planning.** Blessed ground is built on first, forbidden
   ground never, the hall stone is the yard. That gives the patron a canvas without a single
   order, and makes the base the player's.
5. **Do it after the clock.** At today's pace a hut is finished in 30 real seconds and a
   layout costs nothing to walk. With the Dwarf Fortress clock, building is watched and a good
   plan is a better colony.

**The cards, in order.**

| Card | Phase | What it adds |
|---|---|---|
| Settlers save themselves before they build | 2 | Hunger outranks building; the camp moves, or leaves, instead of dying |
| Settlers who plan ahead: the camp's worries, said aloud | 2 | Horizons (winter, beds, the raid) rank the next work; two works at once; the list never ends |
| Skill makes roles: a builder, a hunter, a keeper of the fire | 3 | Skills that grow, seeded by pasts; a named builder whose death is felt |
| Buildings with a job | 3 | Six to eight kinds, each answering one worry and moving one number; sowing, reaping, hunting |
| A village of their own making: a plan that follows the ground | 3 | The town generator as the colony's plan; worn paths; the patron zones by blessing and taboo |
| Buildings rise in stages and grow | 3 | Outline, frame, walls, roof; the hut becomes a hall; raid damage is repaired |
| Each people builds its own way | 3 | Dwarves dig, elves keep the wood, orcs wall first; rules in data; puts the z-levels to work |

**Not yet.** A general planner; a building editor or designations (they would make the player
a foreman); more than eight building kinds; multi-storey interiors. Each is easy to want and
none helps until the seven above are in.

---

## 9. Addendum: monsters, depth and reasons to dig

Added 2026-10-05 after the owner asked for monsters, depth, and reasons to dig down, build
underground, explore and dig into mountains. This is the heart of the game the owner named as
the model (section 7, decision 1), and the world already holds the fiction for it. It is also
about a second game's worth of systems, so order matters more here than anywhere else in this
report. Nine cards follow.

**What exists today.**
- **Settlers cannot leave the surface.** Pathfinding has one node per column, at its floor
  (`src/colony/nav.rs`). There is no digging job; Quarry only breaks boulders and bare rock on
  top.
- **Below every embark is solid rock.** Thirty levels, 60 m, of soil, strata and thin ore
  seams (`src/local/mod.rs`), and nothing else: no cave, no void, no water. The biomes named
  Cave Entrance, Sinkhole, Cenote and Hollow Earth have nothing under them. The world's
  aquifers (1,809 tiles on the dev world) are not used by embarks.
- **Mountains are flat at the scale the game is played.** An embark on Mount Mistvale, "the
  roof of the world" at 10,227 m (seed 42, 256x128, tile 104,104), has about 4 levels of
  relief, 8 m, across its 384 m. The dev mountain (48,18) has one step. Both draw as a blank
  white sheet, and both starve a colony. There is no rock face to dig into.
- **No creature has ever stood on a playable area.** The raid is a hash and a sentence. Yet
  the history holds 132 legendary beasts on the dev world (9 alive), each with a species, a
  size, powers, a lair tile, kills and a hoard; the ecology tracks wolves, bears and lions on
  every tile; and embarks are full of dens and trails with no animal behind them. The
  Shadow's bane lies "in the hoard of Ithiel the Eternal (lair at 27,38)", and there is no
  hoard anywhere to walk into.
- **The underground cannot be read.** A level slice is one flat rock colour with grey blobs
  for ore, in the old atlas tiles. The cross-section exists only as a file written by
  `--local-snapshot`.

**Advice.**

1. **The settlers must want to dig.** With no orders, digging happens only if the camp weighs
   it and chooses it. Four reasons, each a number: warm (no exposure under rock), safe (cannot
   burn, one door to hold), cool (a cellar keeps food three times as long), rich (ore becomes
   tools and arms, richer with depth). And costs: slow, hungry, nothing grows below.
2. **Depth is a dial.** Each level is richer and nearer to something. Every breach is foretold
   a day ahead (damp walls, a hollow ring, claw marks). The miners want the next seam; the
   patron's forbidding hand is the brake. That is a real decision, made without an order.
3. **What is down there is what history left there.** A beast's lair with the hoard it owns by
   name, the tomb of a named dead, the mine of a town that fell, a door sealed in a known year.
   No random dungeon: this world's edge is that everything has a cause.
4. **Few monsters, each readable.** One want, one fear, one sign. Wolves fear fire. The dead
   on a bone field want a name and a burial. What comes up from below uses the stairs the
   colony dug.
5. **If it cannot be seen it does not exist.** The cutaway view and the side section come
   with the first pick, not after. The section of a mountain hold is the picture people will
   post.
6. **A mountain first.** "Dig into the mountain" needs relief at human scale, and a way to
   eat there.

**The cards, in order.**

| Card | Phase | What it adds |
|---|---|---|
| Creatures on the map: the raid arrives on legs | 2 | A creature entity; the attacker walks in from its lair's side and the fight is played; wolves from the dens |
| Mountains at human scale | 4 | Rock faces, ledges and scree on mountain tiles; ibex, snowmelt; the site states its trade |
| Settlers go under | 4 | Three-dimensional paths, a Dig job, rock hardness by kind; stone and ore come out |
| Reasons to dig: warm, safe, cool, rich | 4 | The four numbers and their costs, weighed in the camp's worries |
| Seeing below | 4 | The view follows the settler's level; the underground in ink; a side section |
| Something down there | 4 | Caves, lairs with hoards, tombs, old mines, sealed doors, each with a cause |
| The unknown is blank, and someone has to go in | 4 | The map shows what settlers know; an Explore job for the bold; some do not come back |
| A small bestiary with causes | 4 | Six to eight kinds from the world's own data, at most two on a site |
| Dig too deep | 4 | Depth as risk: foretold breaches, things that come up the stairs, doors that hold |

**Not yet.** Fluids and pressure, cave-ins and supports, magma, mechanisms and minecarts,
generated creature anatomy on the embark, more than eight kinds of monster. Each is where
Dwarf Fortress spent years, and none is needed for the feeling.

**The cost, plainly.** Two of the nine cards are large, and three-dimensional paths touch
every job the settlers have. Nothing here should start before Phase 2's clock: a dig that
finishes in a blink and a monster that crosses the map in 0.2 s would waste the work.

---

## Appendix: to reproduce the main numbers

All deterministic:

```bash
B=./target/release/planet_generator
$B --sim-snapshot dev                      # 30 days: log, decisions, saga, tale, faces
$B --sim-projects 365                      # the steady state after the raid
$B --dev --headless --present              # the loaded present the colony ignores
$B --sim-snapshot coast --tiles-center 12,34   # a rich site that starves in silence
for s in 76 58 23 5 3; do $B --dev --seed $s --sim-snapshot s$s; done   # one script, five worlds
$B --sim-bench 200                         # 2,958 ticks a second
# section 9: the roof of the world is flat at embark scale (see pk_section.png, pk_surface.png)
$B -W 256 -H 128 --seed 42 --no-history --headless --local-snapshot pk --tiles-center 104,104
```

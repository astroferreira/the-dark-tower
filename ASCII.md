# ASCII Character Reference

Complete reference of all ASCII characters used for rendering in the world map, local maps, and simulation.

---

## Harvestable Features (Colonist Debugger)

Features that colonists can harvest for resources:

### Trees (Woodcutter job)
| Char | Feature | Wood Yield |
|------|---------|------------|
| `♣` | Deciduous Tree | 20 |
| `▲` | Conifer Tree | 25 |
| `♠` | Palm Tree | 15 |
| `†` | Dead Tree | 10 |
| `♣` | Jungle Tree | 30 |
| `♣` | Willow Tree | 18 |
| `\|` | Bamboo Clump | 12 |
| `=` | Fallen Log | 8 |
| `~` | Driftwood | 5 |

### Rocks (Miner job)
| Char | Feature | Stone Yield |
|------|---------|-------------|
| `●` | Boulder | 25 |
| `○` | Rock Pile | 15 |
| `●` | Mossy Rock | 10 |
| `▲` | Stalagmite | 12 |
| `◆` | Crystal Cluster | 8 (Crystal) |
| `✦` | Crystal Flower | 3 (Crystal) |
| `◇` | Ice Formation | 10 (Ice) |

### Food Sources (Farmer/Gatherer job)
| Char | Feature | Food Yield | Consumed? |
|------|---------|------------|-----------|
| `✿` | Berry Bush | 8 | No (regrows) |
| `♠` | Mushroom Patch | 6 | No (regrows) |
| `♣` | Herb Patch | 5 | No (regrows) |
| `◇` | Beehive | 4 | No (regrows) |

### Bones (Hunter job)
| Char | Feature | Bone Yield |
|------|---------|------------|
| `☠` | Bone Remains | 5 |
| `☠` | Bone Heap | 12 |

---

## Multi-Tile Building Tiles

Buildings are rendered as multi-tile structures with these tile types:

| Char | Type | Blocks Movement |
|------|------|-----------------|
| `+` | Corner wall | Yes |
| `#` | Wall | Yes |
| `.` | Floor | No |
| `/` | Door | No |
| `=` | Window | Yes |
| `=` | Storage | No |
| `&` | Workstation | No |
| `H` | Ladder | No |
| `%` | Farmland | No |
| `*` | Hearth | No |

### Example Building Layouts
```
Shelter (3x3)     Workshop (5x5)     Gate (3x1)
+-+               +---+              #/#
|.|               |...|
+/+               |.&.|
                  |...|
                  +-/-+
```

---

## Construction Progress

### Unicode (primary)
| Char | Stage | Progress |
|------|-------|----------|
| `○` | Planned | 0% |
| `◔` | Foundation | 1-33% |
| `◑` | Frame | 34-66% |
| `◕` | Finishing | 67-99% |
| `●` | Complete | 100% |

### ASCII Fallback
| Char | Stage | Progress |
|------|-------|----------|
| `o` | Planned | 0% |
| `(` | Foundation | 1-33% |
| `[` | Frame | 34-66% |
| `{` | Finishing | 67-99% |
| `#` | Complete | 100% |

---

## World Map - Biomes

### Water Biomes
| Char | Biome |
|------|-------|
| `~` | Deep Ocean |
| `.` | Ocean |
| `,` | Coastal Water |
| `=` | Lagoon |

### Cold Biomes
| Char | Biome |
|------|-------|
| `#` | Ice |
| `:` | Tundra |
| `B` | Boreal Forest |

### Temperate Biomes
| Char | Biome |
|------|-------|
| `"` | Temperate Grassland |
| `T` | Temperate Forest |
| `R` | Temperate Rainforest |

### Warm Biomes
| Char | Biome |
|------|-------|
| `d` | Desert |
| `;` | Savanna |
| `t` | Tropical Forest |
| `r` | Tropical Rainforest |

### Mountain Biomes
| Char | Biome |
|------|-------|
| `^` | Alpine Tundra |
| `A` | Snowy Peaks |
| `n` | Foothills |

### Wetland Biomes
| Char | Biome |
|------|-------|
| `S` | Swamp |
| `m` | Marsh |
| `&` | Bog |
| `G` | Mangrove Saltmarsh |

### Fantasy Forests
| Char | Biome |
|------|-------|
| `X` | Dead Forest |
| `C` | Crystal Forest |
| `*` | Bioluminescent Forest |
| `M` | Mushroom Forest |
| `P` | Petrified Forest |

### Fantasy Waters
| Char | Biome |
|------|-------|
| `a` | Acid Lake |
| `@` | Lava Lake |
| `o` | Frozen Lake |
| `b` | Bioluminescent Water |

### Wastelands
| Char | Biome |
|------|-------|
| `V` | Volcanic Wasteland |
| `_` | Salt Flats |
| `%` | Ashlands |
| `c` | Crystal Wasteland |

### Ultra-Rare - Ancient/Primeval
| Char | Biome |
|------|-------|
| `Y` | Ancient Grove |
| `W` | Titan Bones |
| `K` | Coral Plateau |

### Ultra-Rare - Geothermal/Volcanic
| Char | Biome |
|------|-------|
| `O` | Obsidian Fields |
| `g` | Geysers |
| `p` | Tar Pits |

### Ultra-Rare - Magical/Anomalous
| Char | Biome |
|------|-------|
| `F` | Floating Stones |
| `Z` | Shadowfen |
| `Q` | Prismatic Pools |
| `N` | Aurora Wastes |

### Ultra-Rare - Desert Variants
| Char | Biome |
|------|-------|
| `D` | Singing Dunes |
| `I` | Oasis |
| `L` | Glass Desert |

### Ultra-Rare - Aquatic
| Char | Biome |
|------|-------|
| `v` | Abyssal Vents |
| `w` | Sargasso |

### Mystical/Supernatural
| Char | Biome |
|------|-------|
| `E` | Ethereal Mist |
| `U` | Starfall Crater |
| `J` | Ley Nexus |
| `H` | Whispering Stones |
| `z` | Spirit Marsh |

### Extreme Geological
| Char | Biome |
|------|-------|
| `u` | Sulfur Vents |
| `l` | Basalt Columns |
| `i` | Painted Hills |
| `j` | Razor Peaks |

### Biological Wonders
| Char | Biome |
|------|-------|
| `h` | Colossal Hive |
| `e` | Bone Fields |
| `y` | Carnivorous Bog |
| `f` | Fungal Bloom |
| `k` | Kelp Towers |

### Exotic Waters
| Char | Biome |
|------|-------|
| `q` | Brine Pools |
| `s` | Hot Springs |
| `0` | Mirror Lake |
| `-` | Ink Sea |
| `+` | Phosphor Shallows |

### Alien/Corrupted
| Char | Biome |
|------|-------|
| `!` | Void Scar |
| `$` | Silicon Grove |
| `(` | Spore Wastes |
| `)` | Bleeding Stone |
| `?` | Hollow Earth |

### Ancient Ruins
| Char | Biome |
|------|-------|
| `[` | Sunken City |
| `]` | Cyclopean Ruins |
| `/` | Buried Temple |
| `\` | Overgrown Citadel |
| `Ω` | Dark Tower |

### Ocean Biomes - Shallow/Coastal
| Char | Biome |
|------|-------|
| `⌇` | Coral Reef |
| `\|` | Kelp Forest |
| `≈` | Seagrass Meadow |

### Ocean Biomes - Mid-depth
| Char | Biome |
|------|-------|
| `─` | Continental Shelf |
| `▲` | Seamount |

### Ocean Biomes - Deep
| Char | Biome |
|------|-------|
| `▼` | Oceanic Trench |
| `░` | Abyssal Plain |
| `═` | Mid-Ocean Ridge |
| `●` | Cold Seep |
| `○` | Brine Pool |

### Ocean Biomes - Fantasy
| Char | Biome |
|------|-------|
| `◆` | Crystal Depths |
| `†` | Leviathan Graveyard |
| `▓` | Drowned Citadel |
| `◎` | Void Maw |
| `◇` | Pearl Gardens |
| `♪` | Siren Shallows |
| `❄` | Frozen Abyss |
| `♨` | Thermal Vents |

### Karst & Volcanic
| Char | Biome |
|------|-------|
| `π` | Cockpit Karst |
| `Θ` | Caldera |
| `∩` | Shield Volcano |
| `△` | Volcanic Cone |
| `▬` | Lava Field |
| `≋` | Fumarole Field |
| `▪` | Volcanic Beach |

---

## Local Map - Terrain

### Ground Types
| Char | Terrain |
|------|---------|
| `.` | Grass |
| `,` | Tall Grass |
| `·` | Dirt |
| `∴` | Sand |
| `░` | Gravel |
| `▓` | Stone |
| `❄` | Snow |
| `═` | Ice |
| `~` | Mud |

### Water Types
| Char | Terrain |
|------|---------|
| `≈` | Shallow Water |
| `▒` | Deep Water |
| `~` | Stream |
| `%` | Marsh |

### Special Ground
| Char | Terrain |
|------|---------|
| `▒` | Volcanic Rock |
| `✧` | Crystal Ground |
| `░` | Ash |
| `░` | Salt |
| `▓` | Lava |
| `▒` | Acid Pool |
| `░` | Frozen Ground |
| `❀` | Coral |
| `░` | Bone |
| `▓` | Obsidian |

---

## Local Map - Features

### Trees
| Char | Feature |
|------|---------|
| `♣` | Deciduous Tree |
| `▲` | Conifer Tree |
| `♠` | Palm Tree |
| `†` | Dead Tree |
| `♣` | Jungle Tree |
| `♣` | Willow Tree |
| `\|` | Bamboo Clump |

### Vegetation
| Char | Feature |
|------|---------|
| `*` | Bush |
| `❀` | Flower Patch |
| `∿` | Fern |
| `¥` | Cactus |
| `\|` | Tall Reeds |
| `♠` | Mushroom Patch |
| `~` | Vine Tangle |
| `○` | Glowing Moss |
| `✦` | Crystal Flower |

### Rocks
| Char | Feature |
|------|---------|
| `●` | Boulder |
| `○` | Rock Pile |
| `◆` | Crystal Cluster |
| `▲` | Stalagmite |
| `◇` | Ice Formation |

### Water Features
| Char | Feature |
|------|---------|
| `○` | Pond |
| `◎` | Spring |
| `◉` | Geyser |

### Structures
| Char | Feature |
|------|---------|
| `□` | Stone Ruin |
| `⌂` | Shrine |
| `◯` | Cave Opening |
| `▮` | Ancient Monolith |
| `☠` | Bone Remains |
| `♨` | Campfire |

### Animal-Related
| Char | Feature |
|------|---------|
| `◎` | Animal Den |
| `○` | Bird Nest |
| `◇` | Beehive |
| `·` | Animal Trail |
| `○` | Watering Hole |
| `•` | Burrow Entrance |

### Civilization Features
| Char | Feature |
|------|---------|
| `†` | Signpost |
| `◎` | Well Structure |
| `═` | Fence Section |
| `†` | Scarecrow |
| `○` | Hay Bale |
| `◉` | Firepit |
| `□` | Storage Shed |
| `▲` | Watch Tower |
| `═` | Bridge |
| `▬` | Dock |

### Colonist-Built Buildings
| Char | Feature |
|------|---------|
| `⌂` | Hut |
| `⌂` | Wooden House |
| `◼` | Stone House |
| `≡` | Farmland |
| `▼` | Mine Entrance |
| `⌂` | Workshop |
| `▣` | Blacksmith |
| `◎` | Granary |
| `▣` | Barracks |
| `▣` | Town Hall |
| `□` | Construction Site |

### Monster Structures
| Char | Feature |
|------|---------|
| `◙` | Monster Lair |
| `◉` | Monster Nest |
| `☠` | Bone Heap |

### Natural Details
| Char | Feature |
|------|---------|
| `=` | Fallen Log |
| `●` | Mossy Rock |
| `▲` | Termite Mound |
| `▴` | Ant Hill |
| `❀` | Wildflowers |
| `✿` | Berry Bush |
| `♣` | Herb Patch |
| `~` | Driftwood |

---

## Elevation Characters

Gradient from Deep Ocean (-4000m) to High Peaks (+4000m):

| Char | Elevation Level |
|------|-----------------|
| `~` | Deep ocean (-4000m) |
| `.` | Ocean floor |
| `-` | Shallow water |
| `=` | Beach/coast |
| `+` | Lowlands |
| `*` | Hills |
| `#` | Mountain foothills |
| `%` | Mountains |
| `^` | High mountains |
| `A` | Alpine peaks |
| `M` | High peaks (+4000m) |

---

## Temperature Characters

Gradient from Cold (-30°C) to Hot (+30°C):

| Char | Temperature |
|------|-------------|
| `#` | Very cold (-30°C) |
| `=` | Cold |
| `-` | Cool |
| `.` | Mild |
| `,` | Warm |
| `;` | Warm |
| `:` | Hot |
| `+` | Very hot |
| `*` | Extreme heat |
| `@` | Extreme heat (+30°C) |

---

## Moisture Characters

Gradient from Dry (0.0) to Wet (1.0):

| Char | Moisture Level |
|------|----------------|
| `_` | Very dry (0.0) |
| `.` | Dry |
| `-` | Dry-moderate |
| `:` | Moderate |
| `;` | Moderate-wet |
| `=` | Wet |
| `+` | Very wet |
| `#` | Very wet |
| `%` | Wet |
| `~` | Very wet (1.0) |

---

## Plate Stress Characters

Gradient from Divergent (-1.0) to Convergent (+1.0):

| Char | Stress Type |
|------|-------------|
| `~` | Strong divergent (rifts) |
| `v` | Moderate divergent |
| `-` | Weak divergent |
| `.` | Neutral |
| `=` | Weak convergent |
| `+` | Moderate convergent |
| `^` | Strong convergent (mountains) |

---

## Entities

### Colonists by Role
| Char | Role |
|------|------|
| `K` | Leader |
| `C` | Champion |
| `P` | Priest |
| `c` | Council Member |
| `s` | Specialist |
| `@` | Citizen (no job) |

### Colonists by Job
| Char | Job |
|------|-----|
| `f` | Farmer |
| `m` | Miner |
| `g` | Guard |
| `W` | Warrior |
| `S` | Scout |
| `w` | Woodcutter |
| `h` | Hunter |
| `F` | Fisher |
| `b` | Builder |
| `H` | Healer |
| `R` | Scholar |
| `A` | Smith |

### Monsters
| Char | Species |
|------|---------|
| `w` | Wolf |
| `B` | Bear |
| `x` | Giant Spider |
| `T` | Troll |
| `G` | Griffin |
| `D` | Dragon |
| `H` | Hydra |
| `b` | Bog Wight |
| `W` | Sandworm |
| `s` | Scorpion |
| `i` | Ice Wolf |
| `Y` | Yeti |
| `Z` | Basilisk |
| `P` | Phoenix |

### Fauna
| Char | Species |
|------|---------|
| `d` | Deer |
| `r` | Rabbit |
| `q` | Squirrel |
| `b` | Boar |
| `f` | Fox |
| `B` | Bison |
| `h` | Horse |
| `E` | Elk |
| `p` | Prairie Dog |
| `g` | Mountain Goat |
| `e` | Eagle |
| `m` | Marmot |
| `a` | Arctic Hare |
| `C` | Caribou |
| `S` | Seal |
| `P` | Penguin |
| `c` | Camel |
| `l` | Lizard |
| `v` | Vulture |
| `F` | Frog |
| `H` | Heron |
| `A` | Alligator |
| `M` | Monkey |
| `t` | Parrot |
| `T` | Tapir |
| `~` | Fish |
| `s` | Salmon |
| `x` | Crab |

---

## Structures

| Char | Structure |
|------|-----------|
| `@` | Town Center (capital) |
| `h` | Hut |
| `H` | Wooden House |
| `s` | Shrine |
| `T` | Temple |
| `f` | Forge |
| `#` | Wall |
| `!` | Watchtower |
| `C` | Castle |
| `&` | Cathedral |
| `%` | Ruins |
| `*` | Monument |

---

## Roads

| Char | Type | Era |
|------|------|-----|
| `.` | Trail | Stone Age |
| `-` | Road | Bronze Age+ |
| `=` | Paved Road | Classical+ |

---

## Debugger Resources

| Char | Resource |
|------|----------|
| `W` | Wood |
| `S` | Stone |
| `I` | Iron Ore |
| `C` | Copper Ore |
| `F` | Food |

---

## Summary

- **Biome Types**: 90+ unique characters
- **Local Terrain**: 25+ terrain types + 50+ features
- **Entities**: 14 monsters, 28 fauna, 12+ colonist types
- **Structures**: 12+ building types
- **Construction**: 10 progress indicators
- **Building Tiles**: 10 tile types

**Total**: 300+ unique ASCII characters

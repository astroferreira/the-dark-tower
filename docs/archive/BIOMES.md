# Biome Reference Guide

This document describes all biomes in the planet generator, their visual characteristics, and spawn conditions.

## Table of Contents
- [Base Biomes](#base-biomes)
- [Fantasy Forests](#fantasy-forests)
- [Desert Variants](#desert-variants)
- [Grassland Variants](#grassland-variants)
- [Cold Region Variants](#cold-region-variants)
- [Volcanic Biomes](#volcanic-biomes)
- [Wetland Variants](#wetland-variants)
- [Mystical Biomes](#mystical-biomes)
- [Alien/Corrupted Biomes](#aliencorrupted-biomes)
- [Ancient Ruins](#ancient-ruins)
- [Special Land Biomes](#special-land-biomes)
- [Fantasy Lakes](#fantasy-lakes)
- [Ocean Biomes - Coastal](#ocean-biomes---coastal)
- [Ocean Biomes - Mid-depth](#ocean-biomes---mid-depth)
- [Ocean Biomes - Deep](#ocean-biomes---deep)
- [Unique Biomes](#unique-biomes)

---

## Base Biomes

These are the foundational biomes determined by temperature, moisture, and elevation. They form the base layer that fantasy biomes replace.

| Biome | Char | Color | Temperature | Moisture | Elevation |
|-------|------|-------|-------------|----------|-----------|
| **Deep Ocean** | `~` | (20, 40, 80) | Any | Any | < -2000m |
| **Ocean** | `.` | (30, 60, 120) | Any | Any | -2000m to -100m |
| **Coastal Water** | `,` | (60, 100, 160) | Any | Any | -100m to 0m |
| **Ice** | `#` | (240, 250, 255) | < -10°C | Any | Land |
| **Tundra** | `:` | (180, 190, 170) | -10°C to 0°C | Any | Land |
| **Boreal Forest** | `B` | (50, 80, 50) | 0°C to 10°C | > 0.5 | Land |
| **Temperate Grassland** | `"` | (140, 170, 80) | 10°C to 20°C | < 0.4 | Land |
| **Temperate Forest** | `T` | (40, 100, 40) | 10°C to 20°C | 0.4 to 0.7 | Land |
| **Temperate Rainforest** | `R` | (30, 80, 50) | 10°C to 20°C | > 0.7 | Land |
| **Desert** | `d` | (210, 180, 120) | > 20°C | < 0.2 | Land |
| **Savanna** | `;` | (170, 160, 80) | > 20°C | 0.2 to 0.4 | Land |
| **Tropical Forest** | `t` | (30, 120, 30) | > 20°C | 0.4 to 0.7 | Land |
| **Tropical Rainforest** | `r` | (20, 90, 40) | > 20°C | > 0.7 | Land |
| **Alpine Tundra** | `^` | (140, 140, 130) | Any | Any | > 1000m, temp < 10°C |
| **Snowy Peaks** | `A` | (255, 255, 255) | Any | Any | > 1500m, temp < -5°C |

---

## Fantasy Forests

Forest biomes with magical or unusual properties. Replace standard forests under specific conditions.

### Mushroom Forest
- **Char:** `M` | **Color:** (140, 80, 160)
- **Description:** Forests dominated by giant fungi and bioluminescent mushrooms
- **Replaces:** Boreal Forest, Temperate Forest, Temperate Rainforest
- **Conditions:** Temperature 5-25°C, Moisture > 0.5
- **Spawn:** 3% chance, clusters of 5

### Crystal Forest
- **Char:** `C` | **Color:** (180, 220, 255)
- **Description:** Trees encased in or replaced by crystalline structures
- **Replaces:** Boreal Forest, Temperate Forest
- **Conditions:** Temperature -10°C to 10°C, Stress > 0.2
- **Spawn:** 2% chance, clusters of 4

### Bioluminescent Forest
- **Char:** `*` | **Color:** (40, 200, 150)
- **Description:** Glowing forests with luminescent flora and fauna
- **Replaces:** Tropical Forest, Tropical Rainforest
- **Conditions:** Temperature > 20°C, Moisture > 0.6
- **Spawn:** 2% chance, clusters of 6

### Petrified Forest
- **Char:** `P` | **Color:** (100, 95, 90)
- **Description:** Ancient forests turned to stone over millennia
- **Replaces:** Boreal Forest, Temperate Forest, Savanna
- **Conditions:** Moisture < 0.4, Stress > 0.15
- **Spawn:** 2.5% chance, clusters of 4

### Dead Forest
- **Char:** `X` | **Color:** (80, 70, 60)
- **Description:** Blackened, lifeless forests killed by volcanic activity
- **Replaces:** Boreal Forest, Temperate Forest, Tropical Forest
- **Conditions:** Stress > 0.3
- **Spawn:** 4% chance, clusters of 5

### Ancient Grove
- **Char:** `Y` | **Color:** (20, 60, 30)
- **Description:** Primeval forest untouched by time, with massive ancient trees
- **Replaces:** Temperate Forest, Temperate Rainforest, Tropical Rainforest
- **Conditions:** Moisture > 0.5, Stress < 0.1
- **Spawn:** 1% chance, clusters of 3

### Silicon Grove
- **Char:** `$` | **Color:** (180, 200, 220)
- **Description:** Alien silicon-based life forms resembling metallic trees
- **Replaces:** Boreal Forest, Temperate Forest
- **Conditions:** Stress > 0.25, Temperature -5°C to 15°C
- **Spawn:** 0.8% chance, clusters of 4

---

## Desert Variants

Specialized desert biomes with unique terrain features.

### Salt Flats
- **Char:** `_` | **Color:** (240, 235, 220)
- **Description:** Brilliant white expanses of dried salt deposits
- **Replaces:** Desert
- **Conditions:** Moisture < 0.2, Elevation -50m to 200m
- **Spawn:** 6% chance, clusters of 8

### Glass Desert
- **Char:** `L` | **Color:** (200, 210, 180)
- **Description:** Sand fused to glass by ancient impacts or volcanism
- **Replaces:** Desert
- **Conditions:** Stress > 0.25
- **Spawn:** 2% chance, clusters of 4

### Singing Dunes
- **Char:** `D` | **Color:** (230, 200, 140)
- **Description:** Sand dunes that produce eerie harmonic sounds in the wind
- **Replaces:** Desert
- **Conditions:** Moisture < 0.15, Stress < 0.15
- **Spawn:** 1.5% chance, clusters of 6

### Oasis
- **Char:** `I` | **Color:** (50, 180, 80)
- **Description:** Verdant patches of life in the desert with fresh water
- **Replaces:** Desert, Savanna
- **Conditions:** Moisture 0.15-0.35, Temperature > 15°C
- **Spawn:** 2% chance, clusters of 2

### Bone Fields
- **Char:** `e` | **Color:** (230, 225, 210)
- **Description:** Ancient graveyards of massive creatures, bones bleaching in the sun
- **Replaces:** Desert, Savanna
- **Conditions:** Moisture < 0.3, Temperature > 10°C
- **Spawn:** 1.2% chance, clusters of 4

---

## Grassland Variants

Unique grassland and savanna variations.

### Fungal Bloom
- **Char:** `f` | **Color:** (200, 100, 180)
- **Description:** Grasslands overtaken by massive fungal growths
- **Replaces:** Temperate Grassland, Savanna
- **Conditions:** Moisture 0.3-0.6, Temperature 10-25°C
- **Spawn:** 2% chance, clusters of 5

### Painted Hills
- **Char:** `i` | **Color:** (200, 140, 100)
- **Description:** Rolling hills with colorful layered sediments
- **Replaces:** Temperate Grassland, Savanna, Desert
- **Conditions:** Moisture < 0.4, Stress > 0.1, Elevation > 100m
- **Spawn:** 1.5% chance, clusters of 5

### Titan Bones
- **Char:** `W` | **Color:** (200, 195, 180)
- **Description:** Remains of impossibly large creatures dotting the landscape
- **Replaces:** Temperate Grassland, Savanna
- **Conditions:** Moisture < 0.4
- **Spawn:** 0.8% chance, clusters of 3

---

## Cold Region Variants

Biomes found in polar and alpine regions.

### Aurora Wastes
- **Char:** `N` | **Color:** (100, 200, 180)
- **Description:** Frozen wastes lit by perpetual aurora displays
- **Replaces:** Tundra, Ice
- **Conditions:** Temperature < -5°C
- **Spawn:** 2% chance, clusters of 6

### Razor Peaks
- **Char:** `j` | **Color:** (100, 95, 105)
- **Description:** Jagged crystalline mountain peaks
- **Replaces:** Alpine Tundra, Snowy Peaks
- **Conditions:** Elevation > 500m, Stress > 0.2
- **Spawn:** 3% chance, clusters of 4

### Whispering Stones
- **Char:** `H` | **Color:** (140, 135, 125)
- **Description:** Wind-carved stone formations that produce haunting sounds
- **Replaces:** Tundra, Alpine Tundra
- **Conditions:** Temperature < 5°C, Stress > 0.1
- **Spawn:** 1.2% chance, clusters of 3

---

## Volcanic Biomes

Biomes formed by volcanic and geothermal activity.

### Volcanic Wasteland
- **Char:** `V` | **Color:** (50, 30, 30)
- **Description:** Active volcanic terrain with lava flows and ash
- **Replaces:** Desert, Savanna, Temperate Grassland
- **Conditions:** Stress > 0.4
- **Spawn:** 8% chance, clusters of 6

### Ashlands
- **Char:** `%` | **Color:** (80, 80, 85)
- **Description:** Lands covered in deep volcanic ash
- **Replaces:** Desert, Savanna, Temperate Grassland, Tundra
- **Conditions:** Stress 0.3-0.5
- **Spawn:** 5% chance, clusters of 5

### Obsidian Fields
- **Char:** `O` | **Color:** (30, 25, 35)
- **Description:** Fields of black volcanic glass
- **Replaces:** Volcanic Wasteland, Ashlands
- **Conditions:** Stress > 0.35
- **Spawn:** 4% chance, clusters of 4

### Sulfur Vents
- **Char:** `u` | **Color:** (200, 180, 60)
- **Description:** Sulfurous fumaroles releasing toxic gases
- **Replaces:** Volcanic Wasteland, Ashlands, Desert
- **Conditions:** Stress > 0.3, Moisture < 0.3
- **Spawn:** 2.5% chance, clusters of 3

### Geysers
- **Char:** `g` | **Color:** (180, 200, 220)
- **Description:** Fields of geothermal geysers and hot springs
- **Replaces:** Tundra, Volcanic Wasteland, Ashlands
- **Conditions:** Stress > 0.2, Moisture > 0.2
- **Spawn:** 2% chance, clusters of 3

### Basalt Columns
- **Char:** `l` | **Color:** (50, 50, 55)
- **Description:** Hexagonal basalt formations from ancient lava flows
- **Replaces:** Volcanic Wasteland, Ashlands, Temperate Grassland
- **Conditions:** Stress > 0.25
- **Spawn:** 2% chance, clusters of 4

---

## Wetland Variants

Swamp, marsh, and bog variations.

### Bog
- **Char:** `&` | **Color:** (90, 70, 50)
- **Description:** Acidic peat bogs with stunted vegetation
- **Base biome** (not a replacement)

### Swamp
- **Char:** `S` | **Color:** (60, 80, 40)
- **Description:** Waterlogged forests with standing water
- **Base biome** (not a replacement)

### Marsh
- **Char:** `m` | **Color:** (80, 100, 60)
- **Description:** Grassy wetlands with shallow water
- **Base biome** (not a replacement)

### Carnivorous Bog
- **Char:** `y` | **Color:** (100, 60, 80)
- **Description:** Bogs filled with carnivorous plants
- **Replaces:** Bog, Marsh, Swamp
- **Conditions:** Moisture > 0.6, Temperature > 10°C
- **Spawn:** 4% chance, clusters of 4

### Shadowfen
- **Char:** `Z` | **Color:** (40, 50, 45)
- **Description:** Dark, perpetually misty wetlands
- **Replaces:** Swamp, Marsh, Bog
- **Conditions:** Moisture > 0.5, Stress > 0.1
- **Spawn:** 2% chance, clusters of 4

### Spirit Marsh
- **Char:** `z` | **Color:** (100, 120, 110)
- **Description:** Haunted marshlands with ethereal phenomena
- **Replaces:** Marsh, Swamp
- **Conditions:** Moisture > 0.5, Temperature < 15°C
- **Spawn:** 1.5% chance, clusters of 4

### Tar Pits
- **Char:** `p` | **Color:** (30, 25, 20)
- **Description:** Natural asphalt seeps trapping unwary creatures
- **Replaces:** Bog, Marsh, Savanna
- **Conditions:** Moisture 0.2-0.5, Stress > 0.15
- **Spawn:** 1.5% chance, clusters of 3

---

## Mystical Biomes

Magical and supernatural locations.

### Ethereal Mist
- **Char:** `E` | **Color:** (180, 190, 210)
- **Description:** Areas of perpetual magical mist
- **Replaces:** Temperate Forest, Boreal Forest, Swamp
- **Conditions:** Moisture > 0.5, Temperature 0-20°C, Stress < 0.15
- **Spawn:** 1% chance, clusters of 5

### Starfall Crater
- **Char:** `U` | **Color:** (90, 60, 120)
- **Description:** Impact sites of magical meteorites
- **Replaces:** Desert, Temperate Grassland, Tundra
- **Conditions:** None (random placement)
- **Spawn:** 0.5% chance, clusters of 3

### Ley Nexus
- **Char:** `J` | **Color:** (200, 180, 255)
- **Description:** Convergence points of magical energy lines
- **Replaces:** Temperate Forest, Temperate Grassland
- **Conditions:** Stress 0.15-0.35
- **Spawn:** 0.6% chance, clusters of 2

### Prismatic Pools
- **Char:** `Q` | **Color:** (180, 200, 220)
- **Description:** Rainbow-colored mineral hot springs
- **Replaces:** Marsh, Swamp, Temperate Grassland
- **Conditions:** Moisture > 0.4, Stress > 0.1
- **Spawn:** 1% chance, clusters of 3

### Floating Stones
- **Char:** `F` | **Color:** (160, 140, 180)
- **Description:** Mysteriously levitating rock formations
- **Replaces:** Alpine Tundra, Temperate Grassland
- **Conditions:** Elevation > 300m, Stress > 0.2
- **Spawn:** 0.8% chance, clusters of 3

---

## Alien/Corrupted Biomes

Strange and otherworldly terrain.

### Void Scar
- **Char:** `!` | **Color:** (20, 10, 30)
- **Description:** Reality-torn areas leaking darkness
- **Replaces:** Desert, Temperate Grassland, Tundra
- **Conditions:** Stress > 0.5
- **Spawn:** 1% chance, clusters of 3

### Spore Wastes
- **Char:** `(` | **Color:** (160, 140, 100)
- **Description:** Lands covered in alien fungal spores
- **Replaces:** Temperate Forest, Boreal Forest, Temperate Grassland
- **Conditions:** Moisture > 0.3, Stress > 0.2
- **Spawn:** 1.2% chance, clusters of 5

### Bleeding Stone
- **Char:** `)` | **Color:** (150, 60, 50)
- **Description:** Red mineral-weeping rock formations
- **Replaces:** Alpine Tundra, Desert, Volcanic Wasteland
- **Conditions:** Stress > 0.3
- **Spawn:** 1% chance, clusters of 3

### Hollow Earth
- **Char:** `?` | **Color:** (60, 50, 45)
- **Description:** Areas with visible cave system entrances
- **Replaces:** Temperate Grassland, Savanna
- **Conditions:** Stress > 0.25, Elevation > 50m
- **Spawn:** 0.8% chance, clusters of 3

---

## Ancient Ruins

Remnants of lost civilizations.

### Sunken City
- **Char:** `[` | **Color:** (70, 90, 110)
- **Description:** Underwater ruins of ancient coastal cities
- **Ocean biome** (appears in shallow ocean)

### Cyclopean Ruins
- **Char:** `]` | **Color:** (110, 105, 95)
- **Description:** Massive stone ruins of unknown origin
- **Replaces:** Temperate Grassland, Savanna, Desert
- **Conditions:** Elevation > 0m
- **Spawn:** 0.8% chance, clusters of 3

### Buried Temple
- **Char:** `/` | **Color:** (170, 150, 120)
- **Description:** Half-buried temples in sand or jungle
- **Replaces:** Desert, Tropical Forest, Tropical Rainforest
- **Conditions:** None (random)
- **Spawn:** 0.6% chance, clusters of 2

### Overgrown Citadel
- **Char:** `\` | **Color:** (60, 90, 50)
- **Description:** Ancient cities reclaimed by forest
- **Replaces:** Temperate Forest, Tropical Forest, Boreal Forest
- **Conditions:** Moisture > 0.4
- **Spawn:** 0.8% chance, clusters of 2

---

## Special Land Biomes

Unique terrain features.

### Colossal Hive
- **Char:** `h` | **Color:** (180, 140, 80)
- **Description:** Massive structures built by giant insects
- **Replaces:** Savanna, Temperate Grassland
- **Conditions:** Temperature > 15°C, Moisture 0.2-0.5
- **Spawn:** 1% chance, clusters of 4

### Sinkhole Lakes
- **Char:** `n` | **Color:** (70, 130, 150)
- **Description:** Lakes formed in collapsed sinkholes
- **Replaces:** Temperate Grassland, Temperate Forest, Tropical Forest
- **Conditions:** Moisture > 0.3, Stress > 0.1
- **Spawn:** 1.5% chance, clusters of 2

### Hot Springs
- **Char:** `s` | **Color:** (100, 180, 190)
- **Description:** Natural geothermal bathing pools
- **Replaces:** Tundra, Temperate Grassland, Boreal Forest
- **Conditions:** Stress > 0.15, Moisture > 0.3
- **Spawn:** 2% chance, clusters of 2

### Crystal Wasteland
- **Char:** `c` | **Color:** (200, 220, 240)
- **Description:** Desert covered in crystalline formations
- **Replaces:** Desert, Salt Flats
- **Conditions:** Moisture < 0.2, Stress > 0.15
- **Spawn:** 2% chance, clusters of 4

---

## Fantasy Lakes

Lakes converted to special biomes based on conditions. **Entire lakes** are converted, not individual tiles.

### Frozen Lake
- **Char:** `o` | **Color:** (200, 230, 250)
- **Description:** Permanently frozen lake surfaces
- **Conversion Condition:** Average temperature < -5°C
- **Chance:** 70% of qualifying lakes

### Lava Lake
- **Char:** `@` | **Color:** (255, 80, 20)
- **Description:** Lakes of molten lava in volcanic regions
- **Conversion Condition:** Average stress > 0.4
- **Chance:** 50% of qualifying lakes

### Acid Lake
- **Char:** `a` | **Color:** (180, 255, 80)
- **Description:** Highly acidic geothermal lakes
- **Conversion Condition:** Stress > 0.2 AND Temperature < 10°C
- **Chance:** 30% of qualifying lakes

### Bioluminescent Water
- **Char:** `b` | **Color:** (50, 180, 200)
- **Description:** Lakes glowing with bioluminescent organisms
- **Conversion Condition:** Temperature > 20°C AND lake size > 10 tiles
- **Chance:** 20% of qualifying lakes

---

## Ocean Biomes - Coastal

Shallow water biomes near coastlines.

### Coral Reef
- **Char:** `⌇` | **Color:** (255, 180, 150)
- **Description:** Tropical coral formations teeming with life
- **Replaces:** Coastal Water
- **Conditions:** Temperature 20-30°C, Stress < 0.15, Depth -100m to -5m
- **Spawn:** 8% chance, clusters of 10

### Kelp Forest
- **Char:** `|` | **Color:** (35, 80, 45)
- **Description:** Dense underwater forests of giant kelp
- **Replaces:** Coastal Water
- **Conditions:** Temperature 8-18°C, Depth -80m to -10m
- **Spawn:** 6% chance, clusters of 8

### Seagrass Meadow
- **Char:** `≈` | **Color:** (50, 120, 70)
- **Description:** Shallow beds of waving seagrass
- **Replaces:** Coastal Water
- **Conditions:** Temperature > 15°C, Depth -50m to -2m
- **Spawn:** 7% chance, clusters of 12

### Pearl Gardens
- **Char:** `◇` | **Color:** (200, 210, 230)
- **Description:** Luminescent pearl-producing shellfish colonies
- **Replaces:** Coastal Water
- **Conditions:** Temperature 18-28°C, Stress < 0.1, Depth -120m to -10m
- **Spawn:** 1.5% chance, clusters of 5

### Siren Shallows
- **Char:** `♪` | **Color:** (100, 180, 200)
- **Description:** Enchanted waters with hypnotic properties
- **Replaces:** Coastal Water
- **Conditions:** Temperature 15-28°C, Depth -80m to -5m
- **Spawn:** 1.2% chance, clusters of 6

---

## Ocean Biomes - Mid-depth

Ocean floor features in moderate depths.

### Continental Shelf
- **Char:** `─` | **Color:** (45, 70, 110)
- **Description:** Flat seafloor with sandy sediment
- **Replaces:** Ocean
- **Conditions:** Stress < 0.15, Depth -300m to -100m
- **Spawn:** 10% chance, clusters of 15

### Seamount
- **Char:** `▲` | **Color:** (60, 50, 80)
- **Description:** Underwater volcanic mountains
- **Replaces:** Ocean
- **Conditions:** Stress > 0.2, Depth -1500m to -300m
- **Spawn:** 4% chance, clusters of 5

### Drowned Citadel
- **Char:** `▓` | **Color:** (80, 90, 100)
- **Description:** Massive sunken ruins of an ancient civilization
- **Replaces:** Ocean, Continental Shelf
- **Conditions:** Depth -500m to -100m, Stress < 0.2
- **Spawn:** 0.8% chance, clusters of 4

### Leviathan Graveyard
- **Char:** `†` | **Color:** (180, 175, 160)
- **Description:** Ancient sea creature bone graveyards
- **Replaces:** Ocean
- **Conditions:** Temperature < 5°C, Stress < 0.1, Depth -1500m to -600m
- **Spawn:** 0.3% chance, clusters of 3

### Sargasso
- **Char:** `w` | **Color:** (60, 100, 50)
- **Description:** Vast floating seaweed masses
- **Replaces:** Ocean
- **Conditions:** Warm calm waters
- **Spawn:** 3% chance, clusters of 12

---

## Ocean Biomes - Deep

Abyssal and hadal zone biomes.

### Oceanic Trench
- **Char:** `▼` | **Color:** (10, 15, 35)
- **Description:** Ultra-deep subduction zone chasms
- **Replaces:** Ocean
- **Conditions:** Stress > 0.35, Depth < -3000m
- **Spawn:** 5% chance, clusters of 8

### Abyssal Plain
- **Char:** `░` | **Color:** (25, 35, 55)
- **Description:** Flat, featureless deep ocean floor
- **Replaces:** Ocean
- **Conditions:** Stress < 0.15, Depth -5500m to -2500m
- **Spawn:** 12% chance, clusters of 20

### Mid-Ocean Ridge
- **Char:** `═` | **Color:** (70, 40, 50)
- **Description:** Divergent plate boundary spreading centers
- **Replaces:** Ocean
- **Conditions:** Divergent stress (-0.3 to -0.1), Depth -4000m to -1500m
- **Spawn:** 6% chance, clusters of 10

### Cold Seep
- **Char:** `●` | **Color:** (40, 50, 45)
- **Description:** Methane-seeping ocean floor areas
- **Replaces:** Ocean, Abyssal Plain
- **Conditions:** Temperature < 6°C, Stress 0.1-0.3, Depth < -1000m
- **Spawn:** 2% chance, clusters of 4

### Brine Pool
- **Char:** `○` | **Color:** (35, 45, 60)
- **Description:** Hypersaline underwater lakes
- **Replaces:** Ocean, Abyssal Plain
- **Conditions:** Temperature < 4°C, Depth < -2000m
- **Spawn:** 1.5% chance, clusters of 3

### Crystal Depths
- **Char:** `◆` | **Color:** (120, 180, 220)
- **Description:** Magical crystalline deep-sea formations
- **Replaces:** Ocean, Abyssal Plain
- **Conditions:** Temperature < 8°C, Stress > 0.2, Depth < -1500m
- **Spawn:** 1% chance, clusters of 5

### Void Maw
- **Char:** `◎` | **Color:** (5, 0, 15)
- **Description:** Reality-torn abyssal holes
- **Replaces:** Ocean, Oceanic Trench
- **Conditions:** Stress > 0.5, Depth < -2500m
- **Spawn:** 0.6% chance, clusters of 3

### Frozen Abyss
- **Char:** `❄` | **Color:** (150, 180, 200)
- **Description:** Ice-covered polar deep waters
- **Replaces:** Ocean
- **Conditions:** Temperature < -10°C, Depth -2500m to -800m
- **Spawn:** 0.8% chance, clusters of 5

### Thermal Vents
- **Char:** `♨` | **Color:** (200, 80, 40)
- **Description:** Hydrothermal vent fields with extremophile life
- **Replaces:** Ocean, Mid-Ocean Ridge
- **Conditions:** Stress > 0.25, Depth < -1500m
- **Spawn:** 2% chance, clusters of 4

---

## Unique Biomes

Biomes that appear exactly **once per map**. Not random - guaranteed placement.

### Dark Tower
- **Char:** `Ω` | **Color:** (25, 20, 30)
- **Description:** A singular ominous tower of dark obsidian, nexus of dark power
- **Placement:** Exactly one per map
- **Location Preference:**
  - High elevation preferred
  - Ancient ruins (+3.0 score)
  - Mountain/alpine areas (+2.5 score)
  - Wastelands and desolate areas (+2.0 score)
  - Mystical locations (+1.5 score)
- **Note:** The Dark Tower is guaranteed to appear on every map, not random

---

## Condition Reference

### Temperature Ranges
- Polar: < -10°C
- Cold: -10°C to 0°C
- Cool: 0°C to 10°C
- Temperate: 10°C to 20°C
- Warm: 20°C to 30°C
- Hot: > 30°C

### Moisture Values (0.0 to 1.0)
- Arid: < 0.2
- Dry: 0.2 to 0.4
- Moderate: 0.4 to 0.6
- Moist: 0.6 to 0.8
- Wet: > 0.8

### Stress Values (Tectonic Activity)
- Calm: < 0.1
- Low: 0.1 to 0.2
- Moderate: 0.2 to 0.3
- High: 0.3 to 0.4
- Extreme: > 0.4
- Divergent: Negative values (spreading ridges)

### Elevation Zones
- Hadal: < -4000m
- Abyssal: -4000m to -2000m
- Deep Ocean: -2000m to -1000m
- Ocean: -1000m to -100m
- Coastal: -100m to 0m
- Lowland: 0m to 500m
- Highland: 500m to 1000m
- Alpine: > 1000m

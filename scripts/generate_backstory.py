#!/usr/bin/env python3
"""Generate the expanded backstory.json data file."""
import json
import os

data = {}

# ============================================================
# COMMON EPITHETS (~300)
# ============================================================
data["common_epithets"] = [
    # Personality
    "the Bold", "the Wise", "the Cruel", "the Just", "the Brave",
    "the Old", "the Young", "the Great", "the Terrible", "the Pious",
    "the Cunning", "the Fair", "the Stern", "the Merciful", "the Silent",
    "the Relentless", "the Resolute", "the Unyielding", "the Benevolent",
    "the Dreaded", "the Magnificent", "the Gentle", "the Fierce",
    "the Patient", "the Wrathful", "the Zealous", "the Humble",
    "the Proud", "the Generous", "the Greedy", "the Jealous",
    "the Ambitious", "the Cautious", "the Reckless", "the Devout",
    "the Faithless", "the Loyal", "the Treacherous", "the Honest",
    "the Deceitful", "the Studious", "the Lazy", "the Diligent",
    "the Paranoid", "the Trusting", "the Suspicious", "the Whimsical",
    "the Somber", "the Cheerful", "the Melancholy", "the Stoic",
    "the Temperate", "the Lustful", "the Chaste", "the Gluttonous",
    "the Abstemious", "the Charitable", "the Miserly", "the Valiant",
    "the Cowardly", "the Stubborn", "the Flexible", "the Obstinate",
    # Physical
    "the Tall", "the Short", "the Scarred", "the Fair-haired",
    "the Dark", "the Pale", "the Red", "the One-eyed", "the Lame",
    "the Strong", "the Frail", "the Mighty", "the Gaunt",
    "the Broad", "the Lean", "the Bearded", "the Bald",
    "the Blind", "the Deaf", "the Twisted", "the Handsome",
    "the Ugly", "the Giant", "the Small", "the Fleet",
    "the Haggard", "the Weathered", "the Ageless",
    # Achievement titles
    "the Conqueror", "the Builder", "the Unifier", "the Peacemaker",
    "the Lawgiver", "the Reformer", "the Liberator", "the Defender",
    "the Avenger", "the Discoverer", "the Explorer", "the Founder",
    "the Restorer", "the Healer", "the Savior", "the Champion",
    "the Victor", "the Triumphant", "the Invincible", "the Undefeated",
    "the Untouchable", "the Unconquered", "the Renowned", "the Celebrated",
    "the Illustrious", "the Glorious", "the Legendary", "the Fabled",
    "the Feared", "the Revered", "the Beloved", "the Honored",
    # Fate/circumstances
    "the Cursed", "the Blessed", "the Exiled", "the Wanderer",
    "the Forsaken", "the Forgotten", "the Doomed", "the Lucky",
    "the Unlucky", "the Lost", "the Returned", "the Risen",
    "the Fallen", "the Reborn", "the Transformed", "the Chosen",
    "the Damned", "the Accursed", "the Haunted", "the Tormented",
    "the Scarred", "the Marked", "the Branded", "the Outcast",
    "the Usurper", "the Pretender", "the Uncrowned",
    # Compound epithets
    "Ironhand", "Goldentongue", "Silvertongue", "Blackheart",
    "Whitecloak", "Stormborn", "Nightwalker", "Dawnbringer",
    "Lightbringer", "Shadowbane", "Oathkeeper", "Oathbreaker",
    "Kinslayer", "Dragonslayer", "Giantslayer", "Wormslayer",
    "Trollbane", "Demonbane", "Witchfinder", "Gravewalker",
    "Sunseeker", "Moonchild", "Stargazer", "Worldshaker",
    "Pathfinder", "Wavebreaker", "Stonebreaker", "Bonecrusher",
    "Skullsplitter", "Shieldbreaker", "Wallbreaker", "Gatekeeper",
    "Warden", "Flamebringer", "Frostbringer", "Stormbringer",
    "Warmonger", "Peaceweaver", "Kingmaker", "Kingslayer",
    "Crowntaker", "Thronetaker", "Scepterbearer", "Lordbane",
    "Ironheart", "Stoneheart", "Coldheart", "Warmheart",
    "Trueheart", "Lionheart", "Wolfheart", "Eagleeye",
    "Hawkeye", "Catseye", "Sharpeye", "Keeneye",
    # Material/color
    "the Golden", "the Silver", "the Iron", "the Bronze",
    "the Copper", "the Steel", "the Adamantine", "the Obsidian",
    "the Crimson", "the Scarlet", "the Azure", "the Emerald",
    "the Sapphire", "the Ruby", "the Diamond", "the Ashen",
    # Nature
    "the Thunderous", "the Stormy", "the Tempestuous", "the Volcanic",
    "the Glacial", "the Frozen", "the Burning", "the Radiant",
    "the Shadowed", "the Verdant", "the Withered", "the Blooming",
    "the Tidal", "the Earthen", "the Windswept",
    # Abstract
    "the Eternal", "the Immortal", "the Undying", "the Everlasting",
    "the Infinite", "the Absolute", "the Supreme", "the Sovereign",
    "the Righteous", "the Holy", "the Profane", "the Sacred",
    "the Unholy", "the Divine", "the Mortal", "the Transcendent",
    "the Enlightened", "the Awakened", "the Ascended",
    # Reputation
    "the Notorious", "the Infamous", "the Famous", "the Anonymous",
    "the Mysterious", "the Enigmatic", "the Inscrutable",
    "the Unpredictable", "the Reliable", "the Dependable",
    "the Unstoppable", "the Inexorable", "the Inevitable",
    "the Merciless", "the Pitiless", "the Compassionate",
    "the Forgiving", "the Vengeful", "the Wrathful",
    # Unique/quirky
    "the Unlikely", "the Improbable", "the Unexpected",
    "the Barefoot", "the Hooded", "the Masked", "the Veiled",
    "the Tattooed", "the Painted", "the Jeweled", "the Crowned",
    "the Unsmiling", "the Laughing", "the Weeping",
    "the Singing", "the Whispering", "the Roaring", "the Howling",
    "the Sleepless", "the Dreaming", "the Wakeful", "the Watchful",
    "Silver-tongued", "Iron-willed", "Stone-faced", "Eagle-eyed",
    "Raven-haired", "Lion-maned", "Wolf-blooded", "Bear-armed",
    "Serpent-tongued", "Dragon-blooded", "Phoenix-born",
    "the Ever-victorious", "the Never-resting", "the All-seeing",
    "the Far-sighted", "the Quick-witted", "the Sharp-minded",
    "the Long-lived", "the Short-tempered", "the Hard-hearted",
    "the Soft-spoken", "the Well-traveled", "the Battle-tested",
    "the War-weary", "the Peace-loving", "the Blood-soaked"
]

# ============================================================
# RACE EPITHETS (30-50 per race)
# ============================================================
data["race_epithets"] = {
    "human": [
        "the Steadfast", "Crownbearer", "the Diplomat", "Shieldwall", "the Commander",
        "the Marshal", "the Senator", "the Tribune", "Battleborn", "the Merchant Prince",
        "the Admiral", "the General", "the Architect", "the Chancellor", "Kingsbane",
        "the Farmer King", "the People's Champion", "the Common", "the Noble",
        "the Knight", "the Paladin", "the Crusader", "the Pilgrim", "the Scholar",
        "the Philosopher", "the Inventor", "the Pioneer", "the Colonist",
        "the Bureaucrat", "the Lawmaker", "Swordmaster", "Bowmaster",
        "the Horseman", "the Sailor", "the Navigator", "the Cartographer"
    ],
    "dwarf": [
        "Stonefist", "Ironbeard", "the Deep Delver", "Goldvein", "Hammersong",
        "the Anvil", "the Tunneler", "Shieldwall", "Forgeborn", "Mountainheart",
        "Gemcutter", "Coalbeard", "Mithrilhand", "the Smelter", "Axegrinder",
        "Tunnelking", "Deepwalker", "Stonecarver", "Runemaster", "Metalshaper",
        "the Beardless", "Ale-drinker", "Barrelchest", "Grudgekeeper", "Oathstone",
        "the Mine-lord", "Cavebreaker", "Crystaleye", "Diamondfist", "Steelforger",
        "the Underking", "Magmaborn", "Greybeard", "Longbeard", "Firebrand",
        "Rockjaw", "Ironbrow", "the Prospector", "Veinfinder", "Copperblood"
    ],
    "elf": [
        "the Starborn", "Moonwhisper", "the Ageless", "Leafsinger", "Dawnbringer",
        "the Luminous", "Sunshadow", "the Eternal Watcher", "Windwalker", "Graceblade",
        "Starweaver", "Moonsinger", "the Twilight", "Dewdancer", "Treewhisper",
        "Sunbeam", "the Evergreen", "Lightfoot", "Starlight", "Silverbranch",
        "the Ancient", "Lorekeeper", "Songweaver", "Dreamwalker", "Nightbloom",
        "Dawnsinger", "Skydancer", "Rootwalker", "Blossomheart", "Vineweaver",
        "the Timeless", "Crystalseer", "Willowbend", "Mistwalker", "Sunfire",
        "the Emerald", "Featherfall", "Springborn", "Autumnleaf", "Winterbough"
    ],
    "orc": [
        "Skullcrusher", "Bloodfang", "the Savage", "Bonecruncher", "Ironjaw",
        "the Butcher", "Doomhammer", "the Merciless", "Warscream", "Goreclaw",
        "Spinebreaker", "Ragebringer", "the Destroyer", "Fleshripper", "Deathblow",
        "Thunderfist", "Battlecry", "Berserker", "Blooddrinker", "Skulltaker",
        "the Unbroken", "Warbringer", "Chainkiller", "Bonegnawer", "Headhunter",
        "Scarsoul", "Redhand", "Gutripper", "Marrow-eater", "the Unstoppable",
        "Pitfighter", "Slavetaker", "the Brutal", "Ironblood", "Ashmaker",
        "Totem-carver", "Warpainter", "the Relentless", "Bonethrone", "the Raider"
    ],
    "goblin": [
        "the Sneak", "Poisonfinger", "Backstabber", "the Rat", "Quickknife",
        "the Slippery", "Shadowbite", "the Schemer", "Wormtongue", "the Trickster",
        "Trapmaster", "Lockpick", "the Vermin", "Cagecutter", "Coin-biter",
        "the Hoarder", "Tunnelrat", "Filchfinger", "the Craven", "Knifethrower",
        "Mushroom-eater", "Muckdweller", "the Survivor", "Foulbreath", "Rubbish-king",
        "the Scrounger", "Scrapmonger", "Nitpicker", "Boltcutter", "Eyegouger",
        "the Cunning", "Shortfuse", "Bombmaker", "Gadgeteer", "the Opportunist",
        "Spiderfriend", "Cavecrawler", "Mudslinger", "Ratcatcher", "the Snitch"
    ],
    "halfling": [
        "the Homely", "Hearthkeeper", "the Well-fed", "Gardener", "the Lucky",
        "Pipesmoker", "Breadmaker", "the Jolly", "Sunwarmer", "Meadowwalker",
        "the Generous", "Ale-brewer", "the Hospitable", "Pantrykeeper", "the Merry",
        "Hillwalker", "Riverswimmer", "Songbird", "the Unexpected", "Footpad",
        "the Thrifty", "Cheesemaker", "the Comfortable", "Storyteller",
        "the Gentle", "Peacekeeper", "the Cordial", "Burrowbuilder", "the Nimble",
        "the Quick", "Fingersmith", "the Resourceful", "the Practical",
        "Mushroom-finder", "the Contented", "Honeybee"
    ],
    "reptilian": [
        "the Venomous", "Scaleborn", "the Cold-blooded", "Sandstrider", "Sunbasker",
        "the Scaled", "Serpent-eye", "Fangbearer", "the Ancient Blood", "Shedding",
        "Tailwhip", "Jawsnapper", "the Coiling", "Heatseeker", "Swampwalker",
        "the Brood-mother", "Egg-guardian", "Tongueflicker", "Dustscale", "Saltblood",
        "the Primordial", "Ambusher", "the Patient Hunter", "Venomspitter",
        "Stonescale", "Ironscale", "Goldscale", "Blackscale", "Firescale",
        "the Saurian", "Clawrend", "Skinkwalker", "the Lurking", "Mudcrawler",
        "the Sun-blessed", "Desertfang", "Marshborn", "Riverjaw", "Oasisfinder"
    ],
    "fey": [
        "the Dreaming", "Thornweaver", "Mistshroud", "the Enchanted", "Moonpetal",
        "the Illusionist", "Dewdancer", "the Whimsical", "Shimmerscale", "Wildbloom",
        "Gossamerweave", "the Tricksome", "Mushroom-king", "Pixiedust",
        "the Mercurial", "Glamourweaver", "Riddlemaker", "Thornking",
        "the Capricious", "Dewdrop", "the Glimmering", "Foxfire",
        "Willowisp", "Spidersilk", "the Fading", "Mirrorborn",
        "the Changeling", "Nightshade", "Briarborn", "Honeyvoice",
        "the Twilight", "Starglimmer", "Moonshroud", "the Beguiling",
        "Dreamspinner", "Cobwebweaver", "the Unseeing", "Riddletongue"
    ],
    "giant": [
        "Earthshaker", "the Mountain", "Thunderstride", "Skyreacher", "the Colossus",
        "Boulderfist", "the Immovable", "Stormcaller", "Peakbreaker", "the Vast",
        "Groundshaker", "Mountainbreaker", "the Towering", "Skycracker", "Cliffwalker",
        "Avalanche", "the Ponderous", "Giantking", "the Titanic", "Stonehurl",
        "Treeuprootter", "Valleymaker", "the Mountainous", "Rockslide",
        "Cloudreacher", "Thundervoice", "the Eternal", "Giantblood",
        "the Ancient One", "Tundrawalker", "Glaciershaper", "Ridgebreaker",
        "the World-born", "Frostbreath", "Peakborn", "the Primeval"
    ],
    "undead": [
        "the Deathless", "Soulstealer", "the Withered", "Gravecaller", "the Eternal",
        "Bonelord", "the Hollow", "Nightbringer", "the Cursed", "Dustwalker",
        "the Lich", "Tombkeeper", "Soulbinder", "the Rotting", "Plaguebringer",
        "Corpsewalker", "the Spectral", "Shadowbound", "Deathwhisper",
        "the Unburied", "Graveborn", "the Decaying", "Skullkeeper", "Wrathwight",
        "Blooddrained", "the Hungering", "Marrowsuckler", "Ghostcaller",
        "the Risen", "Cryptkeeper", "the Pallid", "Boneweaver",
        "Soulharvester", "the Embalmed", "Ashwalker", "the Undying",
        "Tombstalker", "the Necrotic", "Deathgazer", "Spiritbinder"
    ],
    "elemental": [
        "Flameborn", "Stormheart", "Tidecaller", "Earthshaper", "the Living Storm",
        "Magmacore", "Frostessence", "Windwhisper", "the Crystallized",
        "Thundercrash", "Lavaflow", "Glacialheart", "Dustdevil", "Sparkborn",
        "the Elemental", "Coreforged", "Prismborn", "the Primordial",
        "Ashcloud", "Steamborn", "Mudshaper", "Lightningborn",
        "the Volatile", "Emberheart", "Blizzardborn", "Sandsculptor",
        "the Resonant", "Quartzborn", "Ironvein", "the Tempest",
        "Torrentborn", "Geysercaller", "the Crackling", "Obsidianborn"
    ],
    "beastfolk": [
        "Ironpaw", "Swiftfang", "the Alpha", "Howler", "Clawmaster",
        "the Stalker", "Packlord", "Nighthunter", "Bloodscent", "the Feral",
        "Longfang", "Keennose", "Sharphorn", "the Prowler", "Furcloak",
        "Thunderhoof", "Darkpelt", "Silvermane", "Nightfur", "the Tracker",
        "Bonecruncher", "Brindleback", "the Feathered", "Talongrip",
        "Eaglebrow", "Beararms", "Wolfkin", "Foxheart", "the Antlered",
        "Tuskbearer", "Stripetail", "the Spotted", "Scaled-foot",
        "Feathercrest", "Hornbearer", "the Swift", "Windrunner"
    ],
    "construct": [
        "Prime Unit", "the Eternal Machine", "Core Alpha", "the Forged",
        "Gearmaster", "Logic Prime", "the Unerring", "Steelborn",
        "the Calculated", "Overclock", "the Unwavering", "Circuitlord",
        "Data-prime", "the Constructed", "Runecore", "Gearheart",
        "the Automated", "Sparkforged", "the Inevitable Machine",
        "Cogwheel", "Pistonarm", "the Assembler", "Mainspring",
        "the Winding", "Crystalcore", "Ironwork", "the Precision",
        "Pulseborn", "the Architect", "Forgemaster", "the Modular",
        "Obelisk", "the Sentinel", "Monolith", "the Pillar"
    ],
    "_default": []
}

data["race_epithet_chance"] = 0.4

# ============================================================
# RULER TITLES (15-25 per race)
# ============================================================
data["ruler_titles"] = {
    "human": [
        "King", "Queen", "Lord", "Duke", "Emperor", "Sovereign", "Regent",
        "Chancellor", "Imperator", "Patriarch", "Matriarch", "Grand Duke",
        "Prince", "Princess", "Archduke", "Baron", "Count", "Marshal",
        "Consul", "Tribune", "Dictator", "High Lord", "Protector"
    ],
    "dwarf": [
        "Thane", "High King", "Lord Under Mountain", "Forge-Lord", "Iron King",
        "Tunnel-Warden", "Stone-Speaker", "Anvil-Lord", "Deep-King", "Clan-Father",
        "Hammer-Lord", "Mine-King", "Rune-Lord", "Hold-Master", "Grand Thane",
        "Stone-Father", "Vault-Keeper", "Ore-King", "Gem-Lord",
        "High Thane", "Delve-Master", "Under-Lord"
    ],
    "elf": [
        "High Lord", "Archon", "Elder Sovereign", "Star-Lord", "Warden",
        "High Queen", "Lore-Master", "Dream-Lord", "Moon-Sovereign",
        "Sun-Lord", "Grove-Warden", "Star-Sovereign", "Dusk-Lord",
        "Dawn-Lord", "Sylvan Lord", "Tree-Speaker", "Shade-Lord",
        "Wind-Lord", "Elder Lord", "Arch-Warden"
    ],
    "orc": [
        "Warlord", "Warchief", "Overlord", "Blood King", "Skull-Thane",
        "War-Master", "Battle-King", "Rage-Lord", "Iron Fist", "Pain-Lord",
        "Horde-Master", "Skull-King", "Gore-Lord", "Blood-Thane",
        "War-Tyrant", "Carnage-Lord", "Doom-Chief", "Grand Warlord",
        "Fury-King", "Ruin-Lord"
    ],
    "goblin": [
        "Great Boss", "Tyrant", "Despot", "Under-King", "Sneak-Lord",
        "Big Boss", "Supreme Rat", "Tunnel-Tyrant", "Trash-King",
        "Hoard-Master", "Loot-Lord", "Pit Boss", "Gutter-King",
        "Mushroom-King", "Shadow-Boss", "Grand Goblin",
        "Snag-Master", "Filch-Lord", "Muck-King"
    ],
    "halfling": [
        "Mayor", "Burgher", "Elder", "Steward", "Provost",
        "Thain", "Shire-Reeve", "Warden", "Master", "Goodfellow",
        "Head Elder", "Chief Steward", "Grand Mayor", "Land-Warden",
        "Hill-Master", "Dale-Keeper", "Chief Burgher", "High Steward"
    ],
    "reptilian": [
        "Scale-Lord", "Brood-King", "Sun-Sovereign", "Fang-Lord",
        "Serpent-King", "Nest-Lord", "Coil-Master", "Venom-Lord",
        "Scale-Emperor", "Grand Fang", "Clutch-Lord", "Swamp-King",
        "Sand-Lord", "Tide-King", "Sun-Pharaoh", "Saurian Lord",
        "Cold-Lord", "the First Fang", "Scale-Sovereign"
    ],
    "fey": [
        "Faerie Lord", "Dream-Sovereign", "Twilight Monarch", "Archfey",
        "Thorn-Lord", "Mist-Sovereign", "Glamour-King", "Shadow-Prince",
        "Wild-Lord", "Moon-Lord", "Star-Sovereign", "Blossom-Lord",
        "Glade-Keeper", "Dream-Weaver", "Revel-King", "Puck-Lord",
        "Gossamer-Sovereign", "Twilight Lord"
    ],
    "undead": [
        "Lich-Lord", "Death-King", "Bone Sovereign", "Dread Monarch",
        "Crypt-Lord", "Tomb-King", "Wight-Lord", "Necro-Sovereign",
        "Grave-Lord", "Phantom-King", "Shade-Lord", "Soul-Lord",
        "Corpse-King", "Carrion-Lord", "Dust-King", "Specter-Lord",
        "Grand Lich", "Deathless Lord", "the Risen Sovereign"
    ],
    "elemental": [
        "Primarch", "Elemental Lord", "Storm-Sovereign", "Essence-King",
        "Core-Lord", "Prism-Sovereign", "Flame-Lord", "Tide-Lord",
        "Stone-Primarch", "Wind-Lord", "Thunder-Sovereign", "Magma-Lord",
        "Frost-Sovereign", "Crystal-Lord", "Tempest-King"
    ],
    "beastfolk": [
        "Alpha", "Pack-Lord", "Chieftain", "Horn-King", "Fang-Chief",
        "Hunt-Master", "Den-Lord", "Claw-Chief", "Pack-Father",
        "Pride-Lord", "Herd-Master", "Totem-Chief", "Beast-King",
        "Wild-Lord", "Howl-Chief", "Mane-Lord", "Tusk-Chief"
    ],
    "giant": [
        "Titan-Lord", "Mountain-King", "Jarl", "Storm-King", "Stone-Father",
        "Sky-Lord", "Thunder-King", "Peak-Lord", "Giant-King",
        "Colossus-Lord", "World-Shaker", "High Jarl", "Frost-Jarl",
        "Fire-Jarl", "Stone-Jarl", "Elder Giant", "Storm-Jarl"
    ],
    "construct": [
        "Prime Architect", "Core-Sovereign", "Grand Automaton", "Logic-Lord",
        "Forge-Sovereign", "Master Builder", "Code-Lord", "Construct-Prime",
        "Gear-Lord", "Rune-Architect", "Engine-Lord", "Grand Design",
        "Lattice-Lord", "Crystal-Architect", "Iron-Sovereign"
    ],
    "_default": ["King", "Lord", "Ruler", "Sovereign", "Chief", "Elder", "Master"]
}

# ============================================================
# DYNASTY PATTERNS (10-15 per race)
# ============================================================
data["dynasty_patterns"] = {
    "dwarf": [
        "Clan {}", "The {}-forge Line", "House of {}", "The {} Halls",
        "The {}-anvil Clan", "The {} Deepline", "The {} Vein",
        "Clan {} of the Deep", "The {} Stone-blood", "The {}-hammer Line",
        "The {} Hold", "Clan {}-iron", "The {}-gold Lineage"
    ],
    "elf": [
        "House of {}", "The {}-star Line", "The Lineage of {}", "The {} Court",
        "The {}-moon Dynasty", "The {} Treeblood", "The Eternal {} Line",
        "The {}-dawn House", "Court of {}", "The {} Starblood",
        "The {}-leaf Lineage", "The Silvered Line of {}", "House {}-wind"
    ],
    "orc": [
        "The Blood of {}", "Clan {}", "{}'s Horde", "The {} War-Line",
        "{}'s Brood", "The {} Skull-Line", "The {}-fang Clan",
        "The Blood-line of {}", "{}'s War-Breed", "Clan {}-blood",
        "The {} Gore-Line", "{}'s Legion", "The {} Iron-Blood"
    ],
    "goblin": [
        "{}'s Brood", "The {} Gang", "Clan {}", "The {} Clutch",
        "The {} Horde", "{}'s Rats", "The {}-sneak Line",
        "{}'s Crawlers", "The {} Pit-Brood", "Clan {}-fang",
        "The {} Warrens", "{}'s Pack", "The {}-claw Gang"
    ],
    "halfling": [
        "House {}", "The {} Family", "The {}-hill Line",
        "The {} Homestead", "The {} Burrow", "The {}-dale Family",
        "The {} Meadowline", "House {}-green", "The {} Clan",
        "The {}-field Line", "The {} Hillfolk"
    ],
    "reptilian": [
        "The {} Clutch", "Brood of {}", "The {}-scale Line",
        "The {} Nest", "The {}-fang Dynasty", "Brood {}-tooth",
        "The {} Coil-line", "The {}-claw Brood", "Nest of {}",
        "The {} Sun-line", "The {}-venom Dynasty"
    ],
    "fey": [
        "The {} Dream-Line", "Court of {}", "The {} Bloom", "Circle of {}",
        "The {} Glamour-Line", "The {} Thorn-Court", "Ring of {}",
        "The {} Mist-Line", "The {} Star-Court", "The {} Dew-Line",
        "Circle of the {} Moon", "The {} Wild-Court"
    ],
    "undead": [
        "The {} Crypt-Line", "Legacy of {}", "The Eternal {}", "The {} Pact",
        "The {} Bone-Dynasty", "The {} Shadow-Line", "Crypt of {}",
        "The {} Death-Pact", "The Risen Line of {}", "The {} Grave-Dynasty",
        "The {} Dark Legacy", "Tomb of {}"
    ],
    "elemental": [
        "The {} Essence-Line", "Core of {}", "The {} Prism-Line",
        "The {} Elemental Legacy", "Ring of {}", "The {} Storm-Line",
        "The {} Flame-Legacy", "Current of {}", "The {} Crystal-Line"
    ],
    "beastfolk": [
        "Pack of {}", "The {} Blood-Pack", "The {}-fang Pack",
        "Clan {}", "The {} Hunt-Line", "Den of {}", "The {}-claw Clan",
        "The {} Pride", "Herd of {}", "The {} Tooth-Line",
        "The {}-horn Clan"
    ],
    "giant": [
        "The {} Lineage", "Kin of {}", "{}'s Bloodline", "The {} Stone-Line",
        "The {} Peak-Line", "House {}-stone", "The {} Thunder-Line",
        "Kin of {} the Great", "The {}-born Line", "Clan {} the Ancient",
        "The {} Mountain-Blood"
    ],
    "construct": [
        "The {} Design-Line", "Forge of {}", "The {} Core-Line",
        "The {} Logic-Chain", "Pattern of {}", "The {} Engine-Line",
        "Lattice of {}", "The {} Rune-Chain", "Template of {}"
    ],
    "_default": [
        "House of {}", "The {} Dynasty", "The Line of {}", "House {}",
        "The Blood of {}", "The {} Lineage", "The {} Heritage",
        "The {} Royal Line", "Clan {}", "The {} Legacy",
        "The {} Succession", "The {} Bloodline", "Family of {}"
    ]
}

# ============================================================
# CORONATION FOUNDING (8-12 per race)
# ============================================================
data["coronation_founding"] = {
    "dwarf": [
        {"title": "The Founding of {F}", "desc": "{N} struck the first anvil and claimed the mountain halls, becoming the first {T} of {F}."},
        {"title": "{N} declares the founding of {F}", "desc": "With hammer raised and oath sworn before the deep stone, {N} founded {F} and took the title of {T}."},
        {"title": "The First Forging of {F}", "desc": "In the heart of the mountain, {N} lit the Great Forge and declared the founding of {F}."},
        {"title": "The Deep Claim of {N}", "desc": "{N} drove a golden spike into the bedrock, marking the founding of {F} in the ancient tradition."},
        {"title": "{N} opens the First Mine", "desc": "Where others saw only stone, {N} saw riches. The first pickaxe fell and {F} was born."},
        {"title": "The Oath of {N}", "desc": "Before assembled clansfolk, {N} swore the Stone-Oath and became the first {T} of {F}."},
        {"title": "{F} carved from the mountain", "desc": "{N} carved the first hall from living rock, founding {F} with strength and determination."},
        {"title": "The Founding Hammer of {N}", "desc": "{N} forged the Founding Hammer and with its first ring upon the anvil, {F} came into being."},
        {"title": "The Stone-Binding of {F}", "desc": "{N} spoke the words of binding to the mountain itself, and {F} was established under the peaks."},
        {"title": "Rise of the {F} Hold", "desc": "In the deepest cavern, {N} planted the clan banner and declared the founding of {F}, the greatest hold."}
    ],
    "elf": [
        {"title": "The Awakening of {F}", "desc": "Under the starlit canopy, {N} was chosen by the eldest trees to become the first {T} of {F}."},
        {"title": "{N} founds {F}", "desc": "{N} spoke the Words of Binding and wove the first wards, founding {F} in the ancient grove."},
        {"title": "The Planting of {F}", "desc": "With a seed from the World-Tree, {N} planted the first grove and declared the founding of {F}."},
        {"title": "The Starlight Compact", "desc": "Under the light of the eldest stars, {N} gathered the kindred and founded {F} with an oath of starlight."},
        {"title": "The Weaving of {F}", "desc": "{N} wove the first enchantment into the forest, binding land and people as {T} of the new realm of {F}."},
        {"title": "{N} speaks the First Word", "desc": "In the language of trees and rivers, {N} spoke the First Word and {F} blossomed into existence."},
        {"title": "The Moon-Blessing of {F}", "desc": "By the light of the full moon, {N} received the blessing of the ancient spirits and founded {F}."},
        {"title": "The Rootbinding of {N}", "desc": "{N} bound their spirit to the roots of the great tree and became the first {T} of {F}."},
        {"title": "Dawn of {F}", "desc": "As the first light touched the canopy, {N} proclaimed the founding of {F}, a realm of wisdom and beauty."},
        {"title": "The Eternal Song begins", "desc": "{N} began the Eternal Song, a melody that would echo through the ages, and thus {F} was born."}
    ],
    "orc": [
        {"title": "{N} seizes power", "desc": "{N} defeated all challengers in single combat and claimed the title of {T}, founding {F} in blood."},
        {"title": "The Founding of {F}", "desc": "By crushing all rivals, {N} unified the scattered warbands into {F}."},
        {"title": "Rise of {N}", "desc": "{N} raised the war-banner and the tribes rallied, marking the brutal founding of {F}."},
        {"title": "The Blood Oath of {N}", "desc": "{N} cut their palm and swore a blood oath, binding the tribes together as {F} under the new {T}."},
        {"title": "{N} claims the Skull Throne", "desc": "Atop a throne of defeated rivals' skulls, {N} declared the founding of {F}."},
        {"title": "The Unification War of {N}", "desc": "After a campaign of merciless conquest, {N} united the warring clans into {F}."},
        {"title": "{N} sounds the War Horn", "desc": "The great war horn sounded across the plains as {N} declared the birth of {F}."},
        {"title": "The Trial of {N}", "desc": "{N} survived the Trial of Iron and Flame, earning the right to found {F} and rule as {T}."},
        {"title": "The Bloodfire Founding", "desc": "Around a bonfire fed with the weapons of the conquered, {N} proclaimed {F} and was hailed as {T}."},
        {"title": "{N} breaks the old ways", "desc": "{N} slew the old chieftains and forged {F} from their scattered domains."}
    ],
    "human": [
        {"title": "The Founding of {F}", "desc": "{N} gathered followers and established {F}, becoming its first {T}."},
        {"title": "{N} founds {F}", "desc": "With vision and determination, {N} laid the foundations of {F} and was proclaimed its first {T}."},
        {"title": "Rise of {F}", "desc": "From humble beginnings, {N} united the people and declared the founding of {F}."},
        {"title": "The Charter of {F}", "desc": "{N} drafted the Great Charter and with the assent of the nobles, founded {F}."},
        {"title": "The Coronation of {N}", "desc": "In a ceremony witnessed by thousands, {N} was crowned the first {T} of {F}."},
        {"title": "{N} claims the throne of {F}", "desc": "By right of conquest and the will of the people, {N} claimed the throne and founded {F}."},
        {"title": "The Assembly of {F}", "desc": "{N} called the Great Assembly and was elected the first {T} of the newly formed {F}."},
        {"title": "The Compact of {N}", "desc": "{N} forged alliances with the regional lords and united them under the banner of {F}."},
        {"title": "Birth of {F}", "desc": "From the ashes of civil war, {N} built {F} and brought order to the land as its first {T}."},
        {"title": "The Declaration of {N}", "desc": "{N} stood before the gathered masses and declared the founding of {F}, a new era for all."}
    ],
    "_default": [
        {"title": "The Founding of {F}", "desc": "{N} gathered followers and established {F}, becoming its first {T}."},
        {"title": "{N} founds {F}", "desc": "With vision and determination, {N} laid the foundations of {F} and was proclaimed its first {T}."},
        {"title": "Rise of {F}", "desc": "From humble beginnings, {N} united the people and declared the founding of {F}."},
        {"title": "{N} establishes {F}", "desc": "Through strength of will, {N} forged {F} from scattered groups and was named {T}."},
        {"title": "The Proclamation of {N}", "desc": "{N} proclaimed the existence of {F} and was acclaimed its first {T} by the assembled followers."},
        {"title": "The Birth of {F}", "desc": "From nothing, {N} built {F} and took the mantle of {T}, ready to lead into the unknown."},
        {"title": "Dawn of {F}", "desc": "A new dawn broke as {N} declared the founding of {F} and accepted the role of {T}."},
        {"title": "The Gathering of {F}", "desc": "{N} called the scattered peoples together and united them as {F}, taking the title of {T}."},
        {"title": "The First Act of {N}", "desc": "With their first decree, {N} established {F} and was hailed as its founding {T}."},
        {"title": "{N} raises the banner of {F}", "desc": "A new banner flew over the gathered throng as {N} founded {F} and became its {T}."}
    ]
}

# ============================================================
# CORONATION SUCCESSION (25-30 templates)
# ============================================================
data["coronation_succession"] = [
    {"title": "{N} ascends to the throne of {F}", "desc": "Following the passing of {P}, {N} was crowned {T} of {F} in a solemn ceremony."},
    {"title": "Coronation of {N}", "desc": "The elders proclaimed {N} as the new {T} of {F}, successor to {P}."},
    {"title": "{N} inherits rule of {F}", "desc": "By right of blood, {N} inherited the mantle of {T} from {P} and swore the ancient oaths."},
    {"title": "The council chooses {N}", "desc": "After the death of {P}, a council of advisors chose {N} to lead {F} as its next {T}."},
    {"title": "{N} seizes the throne", "desc": "In the turmoil following {P}'s death, {N} moved swiftly to claim the title of {T} of {F}."},
    {"title": "{N} crowned {T} of {F}", "desc": "With the crown of their forebears upon their brow, {N} became {T} of {F}, continuing the legacy of {P}."},
    {"title": "A new {T} for {F}", "desc": "The people of {F} rallied behind {N}, child of {P}, as their new {T}."},
    {"title": "The succession of {N}", "desc": "As was decreed by {P} before their death, {N} assumed the role of {T} of {F}."},
    {"title": "{N} takes the mantle", "desc": "With heavy heart, {N} accepted the mantle of {T} of {F} after the death of {P}."},
    {"title": "Proclamation: {N} rules {F}", "desc": "The heralds proclaimed throughout the land that {N}, heir to {P}, was now {T} of {F}."},
    {"title": "{N} assumes command of {F}", "desc": "In a ceremony both mournful and hopeful, {N} assumed command of {F} as its new {T}."},
    {"title": "The anointing of {N}", "desc": "{N} was anointed with sacred oils and crowned {T} of {F} in the tradition of {P} before them."},
    {"title": "{N} claims the seat of {F}", "desc": "With the blessing of the court, {N} claimed the seat of power and became {T} of {F}."},
    {"title": "Rise of {N} to lead {F}", "desc": "From the shadow of {P}'s death, {N} rose to lead {F} as its new {T}."},
    {"title": "The oath of {N}", "desc": "{N} swore the sacred oath of rulership and became {T} of {F}, honoring the memory of {P}."},
    {"title": "A new era for {F}", "desc": "With {N} upon the throne, a new era began for {F}. The legacy of {P} would not be forgotten."},
    {"title": "{N} receives the crown", "desc": "The crown passed from the lifeless brow of {P} to {N}, who became the new {T} of {F}."},
    {"title": "The investiture of {N}", "desc": "Before the assembled court, {N} was invested as {T} of {F}, successor to the departed {P}."},
    {"title": "{N} answers the call", "desc": "When {P} fell, {N} answered the call of duty and became {T} of {F}."},
    {"title": "The accession of {N}", "desc": "By ancient law and custom, {N} acceded to the title of {T} of {F} upon the death of {P}."},
    {"title": "{N} is declared {T}", "desc": "The nobles of {F} declared {N} to be their new {T}, following the passing of {P}."},
    {"title": "Succession in {F}", "desc": "The line of succession held true as {N} became {T} of {F} after the demise of {P}."},
    {"title": "The crowning of {N}", "desc": "In a ceremony befitting the traditions of {F}, {N} was crowned as the new {T}, heir to {P}."},
    {"title": "{N} fulfills the prophecy", "desc": "As foretold, {N} rose to become {T} of {F} when {P} passed from this world."},
    {"title": "The torch passes to {N}", "desc": "The torch of leadership passed from the fallen {P} to {N}, the new {T} of {F}."}
]

# ============================================================
# DEATH TEMPLATES
# ============================================================
data["death_templates"] = {
    "Natural": [
        {"title": "The passing of {N}", "desc": "{N} died peacefully in their chambers, surrounded by loyal attendants. The people of {F} mourned for a season."},
        {"title": "{S} passes into legend", "desc": "After a long life, {N} breathed their last. {F} observed a year of mourning."},
        {"title": "Death of {N}", "desc": "{N} succumbed to the weight of years, their rule remembered as a time of stability for {F}."},
        {"title": "The final rest of {S}", "desc": "Old and weary, {S} retired to private chambers and never emerged. {F} honored their memory with a grand funeral."},
        {"title": "{S} dies of old age", "desc": "Time claimed what no enemy could. {N} passed away peacefully, leaving {F} to mourn."},
        {"title": "The long sleep of {N}", "desc": "{N} closed their eyes one evening and did not wake. {F} celebrated a life well-lived."},
        {"title": "{N} succumbs to age", "desc": "Despite the best efforts of healers, {N} could not hold back the years. {F} grieved but honored their legacy."},
        {"title": "The quiet end of {S}", "desc": "Without fanfare or drama, {S} passed in their sleep. {F} wept, for they had lost a steady hand."},
        {"title": "{S} meets their ancestors", "desc": "{N} breathed their last with a smile, ready to join their ancestors. {F} held vigil through the night."},
        {"title": "The twilight of {N}", "desc": "As autumn turned to winter, {N} faded like the last leaf. {F} mourned their beloved ruler."},
        {"title": "Farewell to {S}", "desc": "The healers could do nothing more. {N} passed quietly, and {F} entered a period of mourning."},
        {"title": "{N} joins the eternal hall", "desc": "Full of years and honor, {N} departed this world. The people of {F} lined the roads for the funeral procession."}
    ],
    "Battle": [
        {"title": "{N} falls in battle", "desc": "{N} was struck down on the battlefield while leading the forces of {F}. Their body was carried home on their shield."},
        {"title": "The last stand of {N}", "desc": "{N} made a heroic last stand against overwhelming odds, buying time for the retreat of {F}'s armies."},
        {"title": "{S} slain in combat", "desc": "In the chaos of battle, {N} fell to an enemy blade. The warriors of {F} fought bitterly to recover the body."},
        {"title": "Death of {N} at the front", "desc": "{N} refused to command from the rear and paid the ultimate price, falling amid the din of battle."},
        {"title": "{N} dies leading the charge", "desc": "At the head of the charge, {N} was pierced by arrows but fought on until the end. {F} won the battle but lost their leader."},
        {"title": "The fall of {S} in battle", "desc": "{N} was surrounded and overwhelmed by enemy forces. Though they fought like a demon, death found them at last."},
        {"title": "{S} killed in the siege", "desc": "During the siege, an enemy bolt found {N}. The defenders of {F} wailed as their ruler crumpled from the walls."},
        {"title": "{N} makes the ultimate sacrifice", "desc": "{N} threw themselves into the breach, saving the army of {F} but paying with their life."},
        {"title": "The death of {S} in the field", "desc": "The battle was won but {F} paid a terrible price: {N} lay among the fallen, their sword still in hand."},
        {"title": "{N} falls to an ambush", "desc": "{N} was caught in an ambush while marching with the forces of {F}. The surprise attack claimed the life of {F}'s ruler."},
        {"title": "The warrior's end of {S}", "desc": "{N} died as they lived: sword in hand, facing the enemy. {F} honored them with a warrior's funeral."},
        {"title": "{S} mortally wounded in combat", "desc": "{N} was mortally wounded in fierce combat. Though carried from the field, they died before reaching the healers."}
    ],
    "Disease": {
        "templates": [
            {"title": "{N} succumbs to {D}", "desc": "{N} was struck down by {D}. Despite the efforts of healers, the ruler of {F} could not be saved."},
            {"title": "Plague claims {N}", "desc": "{D} claimed {N}, plunging {F} into grief and fear as the sickness spread."},
            {"title": "The sickness of {S}", "desc": "{N} fell ill with {D} and lingered for weeks before death took them. {F} prayed for deliverance."},
            {"title": "{N} felled by {D}", "desc": "The {D} that swept through {F} did not spare even the ruler. {N} died despite every remedy."},
            {"title": "The wasting of {S}", "desc": "{N} wasted away over months as {D} consumed them. The court of {F} watched helplessly."},
            {"title": "{S} dies of {D}", "desc": "No healer could cure {D} when it struck {N}. {F} mourned even as the plague continued."},
            {"title": "The fever takes {N}", "desc": "{D} brought a burning fever that consumed {N}. {F} was left leaderless amid the epidemic."},
            {"title": "{N} lost to pestilence", "desc": "The pestilence of {D} swept through the court of {F}, claiming {N} among its victims."},
            {"title": "The affliction of {S}", "desc": "{N} contracted {D} and suffered greatly before death released them. {F} quarantined the palace."},
            {"title": "{S} perishes from {D}", "desc": "Despite quarantine and the finest medicine, {N} perished from {D}. {F} entered a dark period."},
            {"title": "The last days of {N}", "desc": "Bedridden with {D}, {N} spent their final days dictating orders. {F} carried on their wishes."},
            {"title": "{D} claims the throne", "desc": "In a grim irony, {D} did what no army could: it toppled the ruler of {F}. {N} was dead."}
        ],
        "diseases": [
            "the Crimson Fever", "the Wasting Sickness", "the Grey Pox", "a plague from the east",
            "the Bone Rot", "an unknown malady", "the Shaking Death", "the Blood Cough",
            "the Black Lung", "the Sweating Sickness", "the Red Death", "the Spotted Plague",
            "the Yellow Fever", "the Burning Ague", "the Choking Sickness", "the Sleeping Death",
            "the Weeping Pox", "the Corpse Chill", "the Iron Sickness", "the Moon Madness",
            "a mysterious wasting disease", "the Salt Sickness", "the Rat Plague",
            "the Whiteblood Disease", "the Spore Lung", "the Marsh Fever", "the Tunnel Cough",
            "the Bone-Worm", "the Flesh Decay", "the Mind Fog", "the Blood Thinning",
            "the Gut Rot", "the Skin Blight", "the Eye Cloud", "the Joint Freeze",
            "the Nerve Death", "the Heart Weakness", "the Liver Curse", "the Stone Disease",
            "a virulent pox"
        ]
    },
    "Assassination": [
        {"title": "Assassination of {N}", "desc": "{N} was found dead in their chambers, a poisoned blade beside them. The assassin was never caught, and {F} erupted in suspicion."},
        {"title": "{N} murdered", "desc": "A shadowy conspiracy claimed the life of {N}. The court of {F} descended into paranoia as loyalists hunted for the killers."},
        {"title": "The betrayal of {S}", "desc": "{N} was betrayed by a trusted advisor and murdered in the dead of night. {F} teetered on the brink of chaos."},
        {"title": "{S} poisoned", "desc": "During a feast, {N} was poisoned by an unknown hand. The subsequent purge of the court left {F} diminished."},
        {"title": "A knife in the dark", "desc": "{N} was stabbed while sleeping. The assassin escaped into the night, leaving {F} in turmoil."},
        {"title": "{N} falls to treachery", "desc": "A trusted guard turned on {N}, striking them down in their own throne room. {F} reeled from the treachery."},
        {"title": "The conspiracy against {S}", "desc": "A group of nobles conspired to remove {N} from power. Their plot succeeded, and {F} was plunged into crisis."},
        {"title": "{S} struck down by an arrow", "desc": "While addressing the crowd, {N} was struck by an assassin's arrow. {F} descended into chaos."},
        {"title": "The poisoned cup of {N}", "desc": "{N}'s wine was laced with a slow-acting poison. By the time the symptoms appeared, it was too late for {F}'s ruler."},
        {"title": "{N} ambushed and killed", "desc": "While traveling between cities, {N} was ambushed and killed. The road ran red with blood, and {F} vowed vengeance."},
        {"title": "Treachery claims {S}", "desc": "A servant in the employ of enemies slipped poison into {N}'s food. {F} mourned as suspicion consumed the court."},
        {"title": "The silent death of {N}", "desc": "{N} was found dead without a mark on their body. The healers suspected poison, but {F} could never prove it."}
    ],
    "Duel": [
        {"title": "{N} slain in a duel", "desc": "{N} accepted a challenge of honor and fell to their opponent's blade. The duelists of {F} sang laments for a year."},
        {"title": "The duel of {S}", "desc": "Challenged by a rival claimant, {N} fought with valor but was mortally wounded. {F} buried their ruler with full honors."},
        {"title": "{S} dies by the sword", "desc": "In a dispute over honor, {N} was challenged and slain. The victorious challenger fled the wrath of {F}."},
        {"title": "The fatal challenge", "desc": "{N} could not refuse the challenge and paid with their life. {F} debated whether to avenge or honor the result."},
        {"title": "{N} falls in single combat", "desc": "Before the assembled court of {F}, {N} fought a duel of honor and was slain. The crowd stood in stunned silence."},
        {"title": "The honor-death of {S}", "desc": "{N} chose to settle the dispute through combat rather than negotiation. Their honor was preserved, but their life was not."},
        {"title": "{S} challenged and defeated", "desc": "A champion from a rival faction challenged {N} to single combat. {N} fought bravely but fell. {F} was shaken."},
        {"title": "The trial by combat of {N}", "desc": "{N} insisted on trial by combat to prove their innocence. The gods did not favor them, and {F} lost its ruler."},
        {"title": "{N} duels to the death", "desc": "The grudge between {N} and their rival could only be settled with steel. {N} lost, and {F} was left without a leader."},
        {"title": "The last duel of {S}", "desc": "Age and injury had slowed {S}, but pride would not let them refuse the challenge. {F} mourned a ruler who died on their feet."}
    ],
    "Execution": [
        {"title": "The execution of {N}", "desc": "{N} was tried for crimes against the people and executed. {F} was divided between those who cheered and those who wept."},
        {"title": "{S} put to death", "desc": "After a show trial, {N} was executed by their own court. {F} entered a period of uncertainty."},
        {"title": "The fall of {N}", "desc": "Overthrown by a coalition of nobles, {N} was publicly executed. {F} moved quickly to install a new ruler."},
        {"title": "{N} beheaded by rebels", "desc": "The revolution that swept {F} ended with {N}'s head on a pike. A new order was proclaimed."},
        {"title": "Justice claims {S}", "desc": "For their tyranny, {N} was sentenced to death. The execution was carried out before a jeering crowd in {F}."},
        {"title": "The trial of {N}", "desc": "{N} was convicted of high treason by the council of {F} and put to death. The realm breathed a sigh of relief."},
        {"title": "{S} hanged for tyranny", "desc": "The people of {F} had endured enough. {N} was dragged from the throne and hanged in the public square."},
        {"title": "The downfall of {N}", "desc": "Defeated and captured, {N} was executed by the victorious enemies of {F}."}
    ],
    "Monster": [
        {"title": "{N} devoured by a monster", "desc": "While hunting in the wilderness, {N} was attacked and killed by a fearsome creature. {F} organized a hunt to slay the beast."},
        {"title": "A creature claims {S}", "desc": "{N} ventured too deep into dangerous territory and was slain by a monster. {F} was left leaderless."},
        {"title": "{S} killed by a beast", "desc": "A rampaging creature breached the walls and killed {N} in their own hall. {F} trembled with fear and grief."},
        {"title": "The beast takes {N}", "desc": "On a hunt gone wrong, {N} was ambushed by a monstrous predator. {F} never recovered the body."},
        {"title": "{N} slain by a dragon", "desc": "A great wyrm descended upon {F} and {N} rode out to face it. They did not return."},
        {"title": "The creature's prey: {S}", "desc": "{N} was found slain, the claw marks of some terrible creature upon them. {F} lived in terror."},
        {"title": "{S} falls to a legendary beast", "desc": "The legendary beast that had haunted {F} for generations finally claimed the life of {N}."},
        {"title": "A monster raids {F}", "desc": "During a monster raid on the capital, {N} was killed defending the gates. {F} mourned their fallen ruler."}
    ],
    "Magic": [
        {"title": "{N} consumed by magic", "desc": "A magical experiment went horribly wrong, consuming {N} in arcane fire. {F} banned magical research for a generation."},
        {"title": "Arcane doom claims {S}", "desc": "{N} was struck down by a magical curse that no healer could lift. {F} searched desperately for a cure that never came."},
        {"title": "The magical death of {N}", "desc": "An uncontrolled surge of magical energy killed {N} instantly. The mages of {F} could not explain what happened."},
        {"title": "{S} struck by a curse", "desc": "A dying enemy's curse took root in {N}, slowly draining their life force. {F} watched helplessly as their ruler withered."},
        {"title": "Sorcery kills {N}", "desc": "{N} was the target of powerful hostile magic. Despite wards and protections, the spell found its mark. {F} was devastated."},
        {"title": "The arcane catastrophe", "desc": "An explosion of magical energy destroyed {N}'s chambers and killed {N}. {F} still feels the aftereffects."},
        {"title": "{N} lost to the void", "desc": "While tampering with forbidden magic, {N} was consumed by a portal to the void. {F} sealed the chamber forever."},
        {"title": "The curse upon {S}", "desc": "A terrible curse descended upon {N}, turning their blood to ice. {F} sought a cure but found none."}
    ],
    "Accident": [
        {"title": "The accidental death of {N}", "desc": "{N} died in a tragic accident while inspecting the fortifications of {F}. The scaffolding collapsed without warning."},
        {"title": "{S} killed in an accident", "desc": "A hunting accident claimed {N}'s life. A stray arrow meant for game found the ruler of {F} instead."},
        {"title": "Tragedy strikes {F}", "desc": "{N} drowned while crossing a swollen river during a storm. The body was recovered downstream."},
        {"title": "{N} dies in a fall", "desc": "{N} fell from the tower while surveying the domain. {F} was shocked by the sudden, senseless loss."},
        {"title": "The tragic end of {S}", "desc": "A fire swept through the palace, trapping {N}. Despite rescue efforts, {F} lost its ruler to the flames."},
        {"title": "{S} lost at sea", "desc": "{N}'s ship was lost in a storm. No survivors were found. {F} mourned without a body to bury."},
        {"title": "Misfortune claims {N}", "desc": "While observing the construction of a great monument, falling stones crushed {N}. {F} was left stunned."},
        {"title": "{N} perishes in a cave-in", "desc": "While visiting the mines, a cave-in buried {N}. Despite days of digging, {F} could not save their ruler."}
    ],
    "Suicide": [
        {"title": "The self-destruction of {N}", "desc": "Consumed by guilt and despair, {N} took their own life. {F} was left in shock and sorrow."},
        {"title": "{S} takes their own life", "desc": "Facing defeat and dishonor, {N} chose death on their own terms. {F} debated whether to honor or condemn the act."},
        {"title": "The despair of {N}", "desc": "Driven mad by grief, {N} ended their own existence. {F} mourned both the ruler and the tragedy that drove them to it."},
        {"title": "{N} chooses death over surrender", "desc": "Rather than face capture by the enemy, {N} fell on their own sword. {F} remembered them as defiant to the end."},
        {"title": "The final choice of {S}", "desc": "Beset by enemies within and without, {N} made a terrible choice. {F} was forever changed by the manner of their ruler's death."}
    ],
    "_default": [
        {"title": "The mysterious disappearance of {N}", "desc": "{N} vanished without a trace. Whether they met a hidden fate or chose exile, {F} was left leaderless."},
        {"title": "{S} lost to history", "desc": "The circumstances of {N}'s death remain unknown. Some say they fell to treachery; others whisper of darker fates."},
        {"title": "The unknown fate of {N}", "desc": "{N} embarked on a journey and never returned. {F} searched for years, but no trace was ever found."},
        {"title": "{N} vanishes", "desc": "One day, {N} simply disappeared. No body, no note, no explanation. {F} never learned the truth."},
        {"title": "The mystery of {S}", "desc": "How {N} died remains one of the great mysteries of {F}. Theories abound, but the truth died with them."},
        {"title": "{S} presumed dead", "desc": "After a prolonged absence, {N} was declared dead by the council of {F}. Whether they truly perished, none can say."},
        {"title": "The silence of {N}", "desc": "{N}'s final days were shrouded in secrecy. When the doors were opened, {F} found only an empty chamber."},
        {"title": "The enigma of {S}", "desc": "To this day, the fate of {N} is debated by the scholars of {F}. Some say death; others say exile."}
    ]
}

# ============================================================
# REIGN EVENT TEMPLATES (120-150)
# ============================================================
data["reign_event_templates"] = [
    # BattleFought (15)
    {"event_type": "BattleFought", "title": "The Battle of {PLACE}", "desc": "{RULER} led the {FACTION} to victory against {ENEMY} raiders from the borderlands."},
    {"event_type": "BattleFought", "title": "The Siege of {PLACE}", "desc": "The forces of {FACTION} besieged the fortress of {PLACE}. After a bitter struggle, the walls fell."},
    {"event_type": "BattleFought", "title": "Clash at {PLACE}", "desc": "The armies of {FACTION} met {ENEMY} forces at {PLACE} in a bloody engagement. {NAME} commanded from the vanguard."},
    {"event_type": "BattleFought", "title": "The Rout of the {ENEMY}", "desc": "{RULER} decisively defeated the {ENEMY} at {PLACE}, scattering their forces across the countryside."},
    {"event_type": "BattleFought", "title": "The {ADJ} Campaign", "desc": "{RULER} launched a campaign into {ADJ} territory, winning a string of victories that expanded {FACTION}'s borders."},
    {"event_type": "BattleFought", "title": "Ambush at {PLACE}", "desc": "{ENEMY} forces ambushed the army of {FACTION} near {PLACE}, but {RULER} rallied the troops and turned defeat into victory."},
    {"event_type": "BattleFought", "title": "Battle of the {ADJ} Pass", "desc": "In the narrow passes, {RULER} led {FACTION}'s soldiers against a {ENEMY} army twice their size and prevailed."},
    {"event_type": "BattleFought", "title": "The Sacking of {PLACE}", "desc": "After weeks of siege, {FACTION}'s forces sacked {PLACE}. {RULER} claimed the spoils for the treasury."},
    {"event_type": "BattleFought", "title": "The {ADJ} Slaughter", "desc": "A brutal engagement near {PLACE} saw {FACTION} crush a {ENEMY} warband. {RULER} showed no mercy."},
    {"event_type": "BattleFought", "title": "Defense of {PLACE}", "desc": "When {ENEMY} raiders attacked {PLACE}, {RULER} personally led the defense. The enemy was repulsed with heavy losses."},
    {"event_type": "BattleFought", "title": "The Night Battle of {PLACE}", "desc": "Under cover of darkness, {RULER} launched a surprise attack on the {ENEMY} camp near {PLACE}."},
    {"event_type": "BattleFought", "title": "The River Crossing", "desc": "{RULER} led {FACTION}'s army across a contested river near {PLACE}, defeating {ENEMY} defenders on the far bank."},
    {"event_type": "BattleFought", "title": "Storming of {PLACE}", "desc": "{FACTION}'s warriors stormed the walls of {PLACE} in a daring assault led by {NAME}."},
    {"event_type": "BattleFought", "title": "The {ENEMY} War", "desc": "A bloody war erupted between {FACTION} and the {ENEMY}. Many battles were fought before {RULER} emerged victorious."},
    {"event_type": "BattleFought", "title": "The Pyrrhic Victory at {PLACE}", "desc": "{FACTION} won the battle at {PLACE}, but the cost was staggering. {RULER} mourned the fallen."},

    # Raid (8)
    {"event_type": "Raid", "title": "{ENEMY} raids repelled", "desc": "Under {NAME}'s command, the {FACTION} defended their lands against repeated {ENEMY} incursions."},
    {"event_type": "Raid", "title": "Raiders from the {ADJ} lands", "desc": "{ENEMY} raiders from the {ADJ} territories struck at {FACTION}'s borders. {RULER} organized the defense."},
    {"event_type": "Raid", "title": "The {ENEMY} Incursion", "desc": "A large {ENEMY} raiding party penetrated deep into {FACTION}'s territory before being driven back by {NAME}'s forces."},
    {"event_type": "Raid", "title": "Border skirmishes with {ENEMY}", "desc": "Persistent {ENEMY} raids along the border tested {RULER}'s patience. A punitive expedition was launched."},
    {"event_type": "Raid", "title": "{FACTION} counter-raids the {ENEMY}", "desc": "Tired of constant {ENEMY} raids, {RULER} ordered retaliatory strikes deep into enemy territory."},
    {"event_type": "Raid", "title": "Coastal raids threaten {FACTION}", "desc": "{ENEMY} raiders struck from the sea, burning villages along {FACTION}'s coast. {NAME} built watchtowers in response."},
    {"event_type": "Raid", "title": "The {ADJ} Raid", "desc": "A lightning raid from the {ADJ} frontier caught {FACTION} off guard. {RULER} scrambled to mount a defense."},
    {"event_type": "Raid", "title": "Night raid on {PLACE}", "desc": "{ENEMY} raiders struck {PLACE} under cover of darkness. {RULER} vowed to punish the attackers."},

    # SettlementGrew (6)
    {"event_type": "SettlementGrew", "title": "Expansion of {FACTION}", "desc": "{NAME} ordered the construction of new districts, expanding the capital's reach toward {PLACE}."},
    {"event_type": "SettlementGrew", "title": "New quarter built in {FACTION}", "desc": "Under {RULER}'s direction, a new residential quarter was built to house the growing population of {FACTION}."},
    {"event_type": "SettlementGrew", "title": "The growth of {FACTION}", "desc": "The population of {FACTION} swelled during {NAME}'s reign, necessitating new walls and infrastructure."},
    {"event_type": "SettlementGrew", "title": "Marketplace expansion", "desc": "{RULER} authorized the expansion of the great marketplace, drawing merchants from across the land to {FACTION}."},
    {"event_type": "SettlementGrew", "title": "The {ADJ} District", "desc": "A new district for {ADJ} immigrants was built in {FACTION}'s capital, enriching the city's diversity."},
    {"event_type": "SettlementGrew", "title": "Walls expanded in {FACTION}", "desc": "{RULER} ordered the construction of a new ring of walls to encompass the growing settlements around {FACTION}."},

    # MonumentBuilt (6)
    {"event_type": "MonumentBuilt", "title": "The Great Works of {NAME}", "desc": "{RULER} commissioned grand monuments to celebrate the glory of {FACTION}."},
    {"event_type": "MonumentBuilt", "title": "The {ADJ} Monument", "desc": "A great monument commemorating {FACTION}'s victory over the {ADJ} foe was erected during {RULER}'s reign."},
    {"event_type": "MonumentBuilt", "title": "The Pillar of {NAME}", "desc": "{RULER} erected a great pillar inscribed with the laws and achievements of {FACTION}."},
    {"event_type": "MonumentBuilt", "title": "The Grand Statue", "desc": "A massive statue of {NAME} was carved and placed in the heart of {FACTION}'s capital."},
    {"event_type": "MonumentBuilt", "title": "The Memorial of {PLACE}", "desc": "{RULER} built a memorial at {PLACE} to honor those who fell defending {FACTION}."},
    {"event_type": "MonumentBuilt", "title": "The Great Bridge", "desc": "{RULER} commissioned a magnificent bridge connecting two districts of {FACTION}, a marvel of engineering."},

    # TempleBuilt (6)
    {"event_type": "TempleBuilt", "title": "A temple rises in {FACTION}", "desc": "A great temple was consecrated during the reign of {RULER}, drawing pilgrims from distant lands."},
    {"event_type": "TempleBuilt", "title": "The Grand Cathedral", "desc": "{RULER} completed the construction of a grand cathedral that had been planned for generations."},
    {"event_type": "TempleBuilt", "title": "Shrine of {PLACE}", "desc": "A sacred shrine was built near {PLACE} under {RULER}'s patronage, becoming a center of worship."},
    {"event_type": "TempleBuilt", "title": "The Holy Sanctuary", "desc": "{NAME} donated vast wealth to build a holy sanctuary in {FACTION}, earning the blessing of the clergy."},
    {"event_type": "TempleBuilt", "title": "Restoration of the Old Temple", "desc": "{RULER} restored an ancient temple that had fallen into ruin, reviving the old faith in {FACTION}."},
    {"event_type": "TempleBuilt", "title": "The Monastery of {PLACE}", "desc": "A monastic order was founded near {PLACE} with {RULER}'s blessing, becoming a center of learning."},

    # TreatySigned (6)
    {"event_type": "TreatySigned", "title": "Treaty with the {ADJ} people", "desc": "{NAME} forged an alliance with the {ADJ} people, securing the borders through diplomacy rather than steel."},
    {"event_type": "TreatySigned", "title": "The Peace of {PLACE}", "desc": "After years of conflict, {RULER} signed a peace treaty at {PLACE}, ending hostilities with the {ADJ} faction."},
    {"event_type": "TreatySigned", "title": "The {ADJ} Accord", "desc": "{RULER} negotiated a landmark agreement with the {ADJ} peoples, establishing mutual borders and trade."},
    {"event_type": "TreatySigned", "title": "Diplomatic triumph for {NAME}", "desc": "Through shrewd diplomacy, {NAME} secured a favorable treaty for {FACTION} without shedding a drop of blood."},
    {"event_type": "TreatySigned", "title": "The Non-Aggression Pact", "desc": "{RULER} signed a non-aggression pact with a neighboring power, freeing {FACTION} to focus on internal affairs."},
    {"event_type": "TreatySigned", "title": "Marriage alliance formed", "desc": "{RULER} arranged a marriage alliance with the {ADJ} people, binding the two realms in kinship."},

    # TradeRouteEstablished (5)
    {"event_type": "TradeRouteEstablished", "title": "New trade routes opened", "desc": "Merchants flourished during {NAME}'s reign as new trade routes connected {FACTION} to distant markets."},
    {"event_type": "TradeRouteEstablished", "title": "The {ADJ} Trade Road", "desc": "{RULER} funded the construction of a new road to the {ADJ} lands, boosting commerce in {FACTION}."},
    {"event_type": "TradeRouteEstablished", "title": "Maritime trade expands", "desc": "Under {RULER}'s encouragement, {FACTION}'s merchants established sea routes to far-off ports."},
    {"event_type": "TradeRouteEstablished", "title": "Caravan route to {PLACE}", "desc": "A new caravan route was established between {FACTION} and {PLACE}, bringing exotic goods to the markets."},
    {"event_type": "TradeRouteEstablished", "title": "The Merchant's Guild", "desc": "{RULER} chartered a merchant's guild to organize and protect trade caravans traveling through {FACTION}'s lands."},

    # Plague (6)
    {"event_type": "Plague", "title": "The {PLAGUE}", "desc": "The {PLAGUE} swept through the lands of {FACTION}, claiming many lives. {NAME} rallied the survivors."},
    {"event_type": "Plague", "title": "Pestilence strikes {FACTION}", "desc": "A terrible pestilence fell upon {FACTION} during {RULER}'s reign. The dead were piled in the streets."},
    {"event_type": "Plague", "title": "The Great Sickness", "desc": "The {PLAGUE} killed one in five in {FACTION}. {RULER} imposed quarantine and prayed for deliverance."},
    {"event_type": "Plague", "title": "Epidemic in {FACTION}", "desc": "An epidemic of the {PLAGUE} ravaged the settlements of {FACTION}. {NAME} ordered healers to every corner."},
    {"event_type": "Plague", "title": "The Year of Sickness", "desc": "The year was defined by the {PLAGUE} that consumed {FACTION}. {RULER} survived, but thousands did not."},
    {"event_type": "Plague", "title": "Disease from the {ADJ} lands", "desc": "Travelers from the {ADJ} regions brought the {PLAGUE} to {FACTION}. {RULER} sealed the borders too late."},

    # Drought/Flood/Natural disasters (8)
    {"event_type": "Drought", "title": "The Great Famine", "desc": "A devastating famine struck {FACTION} during the reign of {RULER}. Granaries were emptied and the people suffered greatly."},
    {"event_type": "Drought", "title": "The Dry Years", "desc": "Years of drought parched the lands of {FACTION}. {RULER} rationed water and prayed for rain."},
    {"event_type": "Drought", "title": "Crop failure in {FACTION}", "desc": "The crops withered and died across {FACTION}'s farmlands. {NAME} imported grain at great expense."},
    {"event_type": "Flood", "title": "The Deluge", "desc": "Catastrophic floods ravaged the lowlands of {FACTION}. {NAME} directed the rebuilding efforts."},
    {"event_type": "Flood", "title": "The Great Flood", "desc": "Rivers burst their banks and inundated the heartlands of {FACTION}. {RULER} relocated thousands to higher ground."},
    {"event_type": "Earthquake", "title": "The Great Quake", "desc": "An earthquake shattered buildings and opened chasms in the earth. {RULER} led the recovery of {FACTION}."},
    {"event_type": "VolcanoErupted", "title": "The Eruption", "desc": "A nearby volcano erupted with devastating fury, covering {FACTION}'s lands in ash. {RULER} evacuated the settlements."},
    {"event_type": "Earthquake", "title": "Tremors shake {FACTION}", "desc": "A series of tremors damaged the walls and towers of {FACTION}. {RULER} ordered immediate repairs."},

    # QuestCompleted (6)
    {"event_type": "QuestCompleted", "title": "{NAME} slays the {BEAST}", "desc": "In a legendary feat, {RULER} personally hunted and slew a fearsome {BEAST} that terrorized the countryside."},
    {"event_type": "QuestCompleted", "title": "The Quest of {NAME}", "desc": "{RULER} embarked on a perilous quest to recover {ARTIFACT}, returning triumphant to the cheers of {FACTION}."},
    {"event_type": "QuestCompleted", "title": "{NAME} defeats the {BEAST}", "desc": "Armed with {ARTIFACT}, {RULER} tracked and killed the dreaded {BEAST} of {PLACE}."},
    {"event_type": "QuestCompleted", "title": "The Hero's Return", "desc": "After months of adventure, {RULER} returned to {FACTION} bearing trophies of their victory over the {BEAST}."},
    {"event_type": "QuestCompleted", "title": "{NAME} clears the {ADJ} Lair", "desc": "{RULER} led an expedition into the {ADJ} wilds, destroying a nest of {BEAST}s that had plagued {FACTION}."},
    {"event_type": "QuestCompleted", "title": "The Vanquishing of the {BEAST}", "desc": "The people of {FACTION} celebrated as {RULER} returned with the head of the terrible {BEAST}."},

    # ArtifactFound (5)
    {"event_type": "ArtifactFound", "title": "Discovery of {ARTIFACT}", "desc": "During {NAME}'s reign, explorers uncovered {ARTIFACT}, a relic of immense power from a forgotten age."},
    {"event_type": "ArtifactFound", "title": "{ARTIFACT} unearthed", "desc": "Workers digging foundations unearthed {ARTIFACT}, a mysterious relic that {RULER} claimed for {FACTION}."},
    {"event_type": "ArtifactFound", "title": "The finding of {ARTIFACT}", "desc": "{RULER} commissioned an expedition to {PLACE} that returned with {ARTIFACT}, an artifact of great value."},
    {"event_type": "ArtifactFound", "title": "A relic discovered", "desc": "In the ruins near {PLACE}, scholars discovered {ARTIFACT}. {RULER} declared it a national treasure of {FACTION}."},
    {"event_type": "ArtifactFound", "title": "{ARTIFACT} recovered", "desc": "{RULER} recovered {ARTIFACT} from the lair of a {BEAST}, bringing glory to {FACTION}."},

    # Rebellion/Coup (8)
    {"event_type": "Rebellion", "title": "The {REBEL} Rebellion", "desc": "A noble named {REBEL} raised arms against {RULER}, but the revolt was crushed after a bloody campaign."},
    {"event_type": "Rebellion", "title": "Peasant uprising in {FACTION}", "desc": "The common folk of {FACTION} rose against {RULER}'s taxes. The rebellion was brutally suppressed."},
    {"event_type": "Rebellion", "title": "The {ADJ} Revolt", "desc": "Discontented nobles in the {ADJ} provinces revolted against {RULER}. The rebellion lasted months before being quelled."},
    {"event_type": "Rebellion", "title": "{REBEL} challenges {NAME}", "desc": "{REBEL} declared independence from {FACTION}, forcing {RULER} to march against their own people."},
    {"event_type": "Coup", "title": "Palace conspiracy uncovered", "desc": "A conspiracy to overthrow {NAME} was discovered and the plotters were executed. {RULER} tightened control over the court."},
    {"event_type": "Coup", "title": "Failed coup in {FACTION}", "desc": "A group of generals attempted to seize power from {RULER}. The plot was foiled at the last moment."},
    {"event_type": "Coup", "title": "The Purge of {FACTION}", "desc": "After uncovering a plot against the throne, {RULER} purged the court, executing dozens of suspected traitors."},
    {"event_type": "Coup", "title": "{REBEL} plots against the throne", "desc": "{REBEL}'s conspiracy to overthrow {NAME} was exposed by a loyal spy. The plotters were imprisoned."},

    # Miracle/Religion (8)
    {"event_type": "Miracle", "title": "A miracle in {FACTION}", "desc": "A miraculous event occurred during the reign of {RULER}, strengthening the faith of the people."},
    {"event_type": "Miracle", "title": "The {ADJ} Miracle", "desc": "At {PLACE}, a miraculous healing was witnessed by hundreds. {RULER} declared the site sacred."},
    {"event_type": "Miracle", "title": "Divine sign over {FACTION}", "desc": "A divine sign appeared in the sky above {FACTION}. {RULER} interpreted it as a blessing on their reign."},
    {"event_type": "ReligionFounded", "title": "A new faith arises", "desc": "A prophet emerged during {RULER}'s reign, founding a new religion that spread quickly through {FACTION}."},
    {"event_type": "HolyWarDeclared", "title": "Holy war declared", "desc": "{RULER} declared a holy war against the {ENEMY} infidels, rallying the faithful of {FACTION} to the cause."},
    {"event_type": "CultFormed", "title": "A cult forms in {FACTION}", "desc": "A mysterious cult arose in the shadows of {FACTION} during {RULER}'s reign, worshipping forbidden powers."},
    {"event_type": "CultFormed", "title": "The {ADJ} Cult", "desc": "A secretive cult from the {ADJ} lands gained followers in {FACTION}. {RULER} was unsure whether to tolerate or suppress them."},
    {"event_type": "Miracle", "title": "The Blessed Rain", "desc": "During a terrible drought, rain fell upon {FACTION} at the exact hour {RULER} prayed. The people called it a miracle."},

    # Magic (6)
    {"event_type": "SpellInvented", "title": "Arcane breakthrough", "desc": "Scholars under {NAME}'s patronage made a breakthrough in the arcane arts, advancing the magical knowledge of {FACTION}."},
    {"event_type": "SpellInvented", "title": "New magical discovery", "desc": "The mages of {FACTION} discovered a powerful new form of magic during {RULER}'s reign."},
    {"event_type": "MagicalCatastrophe", "title": "Magical disaster", "desc": "A magical experiment gone wrong devastated a quarter of {FACTION}'s capital. {RULER} banned unsanctioned magic."},
    {"event_type": "MagicalExperiment", "title": "Arcane experiments in {FACTION}", "desc": "{RULER} funded ambitious magical experiments that yielded both wondrous results and terrifying side effects."},
    {"event_type": "CurseApplied", "title": "A curse falls upon {FACTION}", "desc": "A powerful curse was laid upon {FACTION} by a dying sorcerer. {RULER} sought any means to lift it."},
    {"event_type": "CurseLifted", "title": "The curse is broken", "desc": "After years of suffering, the curse upon {FACTION} was finally broken during {RULER}'s reign."},

    # Creatures (8)
    {"event_type": "MonsterRaid", "title": "The {BEAST} Terror", "desc": "A marauding {BEAST} descended upon the outlying villages of {FACTION}, leaving destruction in its wake."},
    {"event_type": "MonsterRaid", "title": "Monsters emerge from the deep", "desc": "Creatures from the underground attacked the settlements of {FACTION}. {RULER} organized a militia to defend the people."},
    {"event_type": "CreatureAppeared", "title": "A {BEAST} spotted near {PLACE}", "desc": "Reports of a {BEAST} near {PLACE} alarmed the people of {FACTION}. {RULER} dispatched scouts."},
    {"event_type": "CreatureSlain", "title": "The {BEAST} of {PLACE} is slain", "desc": "Heroes of {FACTION} tracked and killed the fearsome {BEAST} that had terrorized the region around {PLACE}."},
    {"event_type": "LairEstablished", "title": "A {BEAST} nests near {FACTION}", "desc": "A {BEAST} established its lair dangerously close to {FACTION}'s borders. {RULER} debated whether to attack or leave it be."},
    {"event_type": "CreatureAppeared", "title": "The Awakening", "desc": "An ancient {BEAST} stirred from its long slumber near {PLACE}, threatening the peace of {FACTION}."},
    {"event_type": "MonsterRaid", "title": "{BEAST} rampage", "desc": "A rampaging {BEAST} tore through the farmlands of {FACTION}. {RULER} offered a bounty for its destruction."},
    {"event_type": "CreatureSlain", "title": "Hunters slay the {BEAST}", "desc": "A band of hunters from {FACTION} cornered and slew the dreaded {BEAST} of {PLACE}. {RULER} rewarded them handsomely."},

    # MasterworkCreated/Culture (6)
    {"event_type": "MasterworkCreated", "title": "Golden age of {FACTION}", "desc": "The arts and crafts flourished during {RULER}'s reign. Master artisans produced works of legendary quality."},
    {"event_type": "MasterworkCreated", "title": "A masterpiece created", "desc": "A master artisan created a work of extraordinary beauty during {RULER}'s reign, bringing fame to {FACTION}."},
    {"event_type": "MasterworkCreated", "title": "The {ADJ} Masterwork", "desc": "Inspired by {ADJ} traditions, craftsmen of {FACTION} created a masterwork that would be admired for generations."},
    {"event_type": "MasterworkCreated", "title": "The Great Library", "desc": "{RULER} founded a great library in {FACTION}, gathering knowledge from across the known world."},
    {"event_type": "MasterworkCreated", "title": "Artistic renaissance in {FACTION}", "desc": "Under {RULER}'s patronage, {FACTION} experienced an artistic renaissance. Poets, painters, and sculptors flourished."},
    {"event_type": "MasterworkCreated", "title": "The {FACTION} School of Craft", "desc": "{RULER} established a school of craft and artistry that trained the finest makers in the land."},

    # WarDeclared (6)
    {"event_type": "WarDeclared", "title": "War of the {ADJ} Border", "desc": "{RULER} declared war on the {ADJ} clans over disputed territory. The conflict raged for years."},
    {"event_type": "WarDeclared", "title": "War against the {ENEMY}", "desc": "{RULER} declared war on the {ENEMY}, rallying {FACTION}'s armies for a decisive campaign."},
    {"event_type": "WarDeclared", "title": "The {ADJ} War", "desc": "Tensions with the {ADJ} peoples boiled over into open war during {RULER}'s reign."},
    {"event_type": "WarDeclared", "title": "Invasion of the {ADJ} lands", "desc": "{RULER} launched an invasion of the {ADJ} territories, seeking to expand {FACTION}'s domain."},
    {"event_type": "WarDeclared", "title": "Defensive war declared", "desc": "When the {ENEMY} massed on the border, {RULER} had no choice but to declare war to defend {FACTION}."},
    {"event_type": "WarDeclared", "title": "The War of Retribution", "desc": "Seeking vengeance for past wrongs, {RULER} declared a war of retribution against the {ENEMY}."},

    # AllianceFormed/Diplomacy (5)
    {"event_type": "AllianceFormed", "title": "Alliance with the {ADJ} people", "desc": "{RULER} forged a military alliance with the {ADJ} people, strengthening {FACTION}'s position."},
    {"event_type": "AllianceFormed", "title": "The Grand Alliance", "desc": "{RULER} organized a grand alliance of multiple factions to face a common threat."},
    {"event_type": "AllianceFormed", "title": "Pact of mutual defense", "desc": "{RULER} signed a pact of mutual defense with neighboring powers, ensuring {FACTION}'s security."},
    {"event_type": "AllianceFormed", "title": "The {ADJ} Coalition", "desc": "{RULER} formed a coalition with the {ADJ} peoples to counter a growing threat from the {ENEMY}."},
    {"event_type": "AllianceFormed", "title": "Bond of friendship", "desc": "{RULER} and the {ADJ} leader swore a bond of friendship, cementing peaceful relations for {FACTION}."},

    # Siege (4)
    {"event_type": "SiegeBegun", "title": "Siege of {PLACE}", "desc": "{RULER} besieged {PLACE}, encircling the fortress with {FACTION}'s armies. The defenders held for months."},
    {"event_type": "SiegeBegun", "title": "The Blockade of {PLACE}", "desc": "{RULER} ordered a blockade of {PLACE}, starving the defenders into eventual submission."},
    {"event_type": "SiegeBegun", "title": "{FACTION} besieged", "desc": "The {ENEMY} laid siege to {FACTION}'s capital. {RULER} rallied the defenders for a long resistance."},
    {"event_type": "SiegeBegun", "title": "The Long Siege", "desc": "The siege of {PLACE} by {FACTION}'s forces lasted over a year. {RULER} refused to relent."},

    # Massacre (3)
    {"event_type": "Massacre", "title": "The Massacre of {PLACE}", "desc": "In a dark chapter of {FACTION}'s history, {RULER}'s forces massacred the defenders of {PLACE}."},
    {"event_type": "Massacre", "title": "The {ADJ} Purge", "desc": "{RULER} ordered a purge of {ENEMY} sympathizers within {FACTION}. The resulting bloodshed shocked the populace."},
    {"event_type": "Massacre", "title": "Bloodbath at {PLACE}", "desc": "What began as a battle at {PLACE} devolved into a massacre. {RULER} later expressed regret, but the damage was done."},

    # PopulationMigrated (3)
    {"event_type": "PopulationMigrated", "title": "Migration to {FACTION}", "desc": "Refugees from the {ADJ} lands migrated to {FACTION} during {RULER}'s reign, swelling the population."},
    {"event_type": "PopulationMigrated", "title": "The Great Migration", "desc": "A large group of settlers arrived in {FACTION}'s territory, seeking land and opportunity under {RULER}'s rule."},
    {"event_type": "PopulationMigrated", "title": "Exodus from {FACTION}", "desc": "Harsh conditions during {RULER}'s reign caused many to flee {FACTION}, seeking better lives elsewhere."},

    # Other/Miscellaneous (6)
    {"event_type": "Other", "title": "Census of {FACTION}", "desc": "{RULER} ordered the first comprehensive census of {FACTION}, revealing a population larger than expected."},
    {"event_type": "Other", "title": "The {ADJ} Festival", "desc": "{RULER} established a grand festival celebrating {FACTION}'s culture, drawing visitors from far and wide."},
    {"event_type": "Other", "title": "Reform of the laws", "desc": "{RULER} reformed the laws of {FACTION}, establishing a more just and organized system of governance."},
    {"event_type": "Other", "title": "The Great Road", "desc": "{RULER} commissioned the construction of a great road connecting {FACTION}'s major settlements."},
    {"event_type": "Other", "title": "Discovery of {PLACE}", "desc": "Explorers from {FACTION} discovered fertile new lands near {PLACE} during {RULER}'s reign."},
    {"event_type": "Other", "title": "Comet over {FACTION}", "desc": "A great comet appeared in the sky during {RULER}'s reign. The people debated whether it was an omen of good or ill."},
]

# ============================================================
# ENEMY NAMES (15-25 per race)
# ============================================================
data["enemy_names"] = {
    "dwarf": [
        "goblin", "orc", "troll", "dark elf", "drake", "kobold",
        "cave spider", "deep worm", "ogre", "giant rat", "gnoll",
        "troglodyte", "duergar", "derro", "grimlok", "rust monster",
        "shadow creature", "fire beetle", "stone golem", "cave troll"
    ],
    "elf": [
        "orc", "troll", "undead", "dark fey", "spider-kin", "goblin",
        "blight creature", "corrupted treant", "necromancer", "shadow beast",
        "gnoll", "ogre", "warg", "harpy", "drow", "cultist",
        "blighted animal", "corrupted elemental", "cave troll", "wraith"
    ],
    "orc": [
        "human", "elf", "dwarf", "rival orc", "ogre", "troll",
        "gnoll", "centaur", "griffin rider", "holy warrior", "paladin",
        "militia", "mercenary", "dragon", "giant", "elemental",
        "spirit guardian", "war golem", "border patrol", "knight"
    ],
    "goblin": [
        "dwarf", "human", "rival goblin", "kobold", "gnoll", "rat swarm",
        "cave spider", "adventurer", "bounty hunter", "wolf pack",
        "militia", "guard patrol", "eagle", "owl bear", "hobgoblin",
        "lizardman", "skeleton", "slime", "trapper", "ranger"
    ],
    "halfling": [
        "bandit", "wolf", "fox", "goblin", "orc raider", "troll",
        "wild boar", "giant rat", "snake", "hawk", "raven",
        "tax collector", "bully", "thief", "brigand", "outlaw"
    ],
    "reptilian": [
        "mammal-folk", "bird-folk", "amphibian", "rival tribe", "warm-blood",
        "marsh predator", "swamp thing", "giant crocodile", "sea serpent",
        "adventurer", "treasure hunter", "colonist", "missionary",
        "frost creature", "ice elemental", "northern raider"
    ],
    "fey": [
        "undead", "iron-wielder", "shadow creature", "blighted beast", "mortal",
        "cold iron knight", "necromancer", "blight lord", "corrupted spirit",
        "demon", "devil", "abomination", "industrial", "machine",
        "crystal golem", "void creature", "entropy beast", "death mage"
    ],
    "undead": [
        "paladin", "cleric", "living", "radiant fey", "exorcist",
        "holy warrior", "sun priest", "life mage", "blessed knight",
        "angel", "celestial", "divine champion", "purifier",
        "inquisitor", "templar", "sacred guardian", "light bearer"
    ],
    "elemental": [
        "void creature", "anti-magic user", "null mage", "entropy beast",
        "abyssal demon", "planar invader", "chaos beast", "order knight",
        "binding mage", "summoner", "elemental rival", "soul trap",
        "crystal shatter", "storm hunter"
    ],
    "beastfolk": [
        "poacher", "trapper", "hunter", "civilized soldier", "mage",
        "undead", "mechanical construct", "slaver", "deforester",
        "fire elemental", "ice creature", "corrupted beast",
        "fell spirit", "parasite swarm", "plague rat"
    ],
    "giant": [
        "dragon", "titan", "rival giant", "swarm of small-folk",
        "siege engine", "flying creature", "mage tower",
        "mountain beast", "ice wyrm", "thunder bird",
        "burrowing horror", "elemental titan", "colossus"
    ],
    "construct": [
        "rust monster", "lightning elemental", "acid creature",
        "entropy being", "void touch", "dispel mage", "anti-magic field",
        "corrosion beast", "electromagnetic storm", "nullifier",
        "override signal", "logic virus", "rogue construct"
    ],
    "human": [
        "barbarian", "bandit", "pirate", "orc raider", "goblin",
        "troll", "undead", "dragon", "demon", "rival kingdom",
        "nomad horde", "corsair", "cultist", "heretic", "rebel",
        "mercenary band", "warlord", "monster", "beast", "marauder"
    ],
    "_default": [
        "barbarian", "bandit", "marauder", "pirate", "raider", "nomad",
        "warlord", "brigand", "outlaw", "corsair", "invader",
        "pillager", "freebooter", "plunderer", "despoiler",
        "ravager", "looter", "highwayman", "cutthroat", "vandal"
    ]
}

# ============================================================
# FACTION ADJECTIVES (100+)
# ============================================================
data["faction_adjectives"] = [
    # Geographic
    "northern", "southern", "eastern", "western", "highland", "lowland",
    "river", "mountain", "forest", "coastal", "desert", "marsh",
    "island", "valley", "plateau", "tundra", "steppe", "canyon",
    "lakeside", "underground", "subterranean", "arctic", "tropical",
    "volcanic", "glacial", "continental", "maritime", "riverine",
    "upland", "moorland", "fenland", "woodland", "prairie",
    # Colors/Materials
    "iron", "golden", "silver", "storm", "shadow", "frost",
    "crimson", "scarlet", "azure", "emerald", "obsidian", "jade",
    "ivory", "ebony", "amber", "copper", "bronze", "steel",
    "crystal", "diamond", "ruby", "sapphire", "onyx", "opal",
    "ashen", "pearl", "coral", "granite", "marble", "slate",
    # Nature
    "thunder", "lightning", "wind", "rain", "snow", "ice",
    "fire", "flame", "sun", "moon", "star", "dawn",
    "dusk", "twilight", "midnight", "autumn", "winter", "spring",
    "summer", "harvest", "tidal", "tempest", "gale", "breeze",
    # Quality/Character
    "proud", "fallen", "broken", "ancient", "eternal", "sacred",
    "cursed", "blessed", "forgotten", "lost", "hidden", "secret",
    "dark", "bright", "pale", "fierce", "gentle", "wild",
    "free", "bound", "wandering", "roaming", "settled", "exiled",
    "united", "divided", "conquered", "unconquered", "resilient",
    "defiant", "loyal", "rebel", "outcast", "noble", "savage",
    "peaceful", "warlike", "devout", "heretical"
]

# ============================================================
# PLAGUE NAMES (80+)
# ============================================================
data["plague_names"] = [
    "Crimson Fever", "Grey Pox", "Bone Rot", "Wasting Sickness",
    "Blood Cough", "Shaking Death", "Pale Plague", "Shadow Blight",
    "Iron Sickness", "Weeping Pox", "Rat Fever", "Spore Lung",
    "Corpse Chill", "Night Sweats", "Scale Rot", "Moon Madness",
    "Black Lung", "Red Death", "Yellow Blight", "Green Rot",
    "White Plague", "Blue Fever", "Purple Pox", "Brown Wasting",
    "Sweating Sickness", "Spotted Plague", "Burning Ague",
    "Choking Sickness", "Sleeping Death", "Salt Sickness",
    "Marsh Fever", "Tunnel Cough", "Bone-Worm", "Flesh Decay",
    "Mind Fog", "Blood Thinning", "Gut Rot", "Skin Blight",
    "Eye Cloud", "Joint Freeze", "Nerve Death", "Heart Weakness",
    "Liver Curse", "Stone Disease", "Creeping Numbness",
    "Scarlet Tremors", "Ashen Wasting", "Frost Bite Plague",
    "Whiteblood Disease", "Blackvein Plague", "Ember Fever",
    "Crystal Sickness", "Dust Lung", "Void Chill", "Star Plague",
    "Moon Blight", "Sun Sickness", "Thunder Ague", "Storm Fever",
    "Earthquake Sickness", "Magma Pox", "Glacier Disease",
    "Forest Blight", "Desert Wasting", "Ocean Fever", "River Rot",
    "Mountain Sickness", "Cave Plague", "Sky Blight", "Root Rot",
    "Petal Plague", "Thorn Sickness", "Mushroom Madness",
    "Swamp Gas Disease", "Quicksand Fever", "Lava Burns",
    "Frostfire Plague", "Shadowveil Sickness", "Dreamrot",
    "Soulwasting", "Mindfire", "the Creeping Doom"
]

# ============================================================
# BEAST NAMES (100+)
# ============================================================
data["beast_names"] = [
    "wyrm", "troll", "giant spider", "dire wolf", "basilisk",
    "chimera", "wyvern", "manticore", "hydra", "drake",
    "griffon", "cockatrice", "behemoth", "kraken",
    "thunderbird", "shadow stalker", "bone golem", "cave bear",
    "frost giant", "fire elemental", "swamp thing", "barrow wight",
    "dragon", "serpent", "leviathan", "phoenix", "cerberus",
    "minotaur", "cyclops", "harpy", "medusa", "sphinx",
    "gorgon", "centaur", "satyr", "naga", "banshee",
    "wraith", "specter", "revenant", "lich", "vampire",
    "werewolf", "wendigo", "yeti", "sasquatch", "mothman",
    "roc", "thunderworm", "sand wurm", "ice serpent", "fire drake",
    "storm elemental", "earth golem", "water weird", "air elemental",
    "shadow demon", "pit fiend", "imp swarm", "hell hound",
    "nightmare", "death knight", "skeletal dragon", "zombie horde",
    "ghoul pack", "mummy lord", "flesh golem", "iron golem",
    "stone colossus", "crystal spider", "obsidian scorpion",
    "lava toad", "frost wurm", "thunder lizard", "plague rat swarm",
    "giant wasp", "giant scorpion", "giant centipede", "cave crawler",
    "deep horror", "mind flayer", "beholder", "aboleth",
    "owlbear", "displacer beast", "rust monster", "gelatinous cube",
    "shambling mound", "treant", "giant eagle", "giant boar",
    "dire bear", "dire tiger", "giant elk", "mammoth",
    "terror bird", "giant crab", "giant octopus", "sea serpent",
    "giant bat", "carrion crawler", "hook horror", "purple worm"
]

# ============================================================
# SUCCESSION TEMPLATES (20+)
# ============================================================
data["succession_templates"] = [
    {"title": "Coronation of {N}", "desc": "After the death of {DEAD}, {N} was crowned as the new ruler of {F}."},
    {"title": "{N} ascends to lead {F}", "desc": "{N} inherited the mantle of leadership from {DEAD}, taking command of {F}."},
    {"title": "Succession in {F}", "desc": "With {DEAD} gone, the council proclaimed {N} as the new ruler of {F}."},
    {"title": "{N} takes the throne of {F}", "desc": "By right of blood, {N} claimed the throne left vacant by {DEAD}'s death."},
    {"title": "The heir of {DEAD}", "desc": "{N}, heir of {DEAD}, assumed control of {F} amid a period of mourning."},
    {"title": "A new ruler for {F}", "desc": "The people of {F} accepted {N} as their new ruler following the death of {DEAD}."},
    {"title": "{N} inherits {F}", "desc": "As {DEAD}'s chosen successor, {N} took the reins of power in {F}."},
    {"title": "Power passes to {N}", "desc": "With {DEAD}'s passing, power in {F} passed to {N}, who swore to honor the legacy."},
    {"title": "The succession of {N} in {F}", "desc": "The transition of power from {DEAD} to {N} in {F} was smooth, thanks to careful planning."},
    {"title": "{N} chosen to lead {F}", "desc": "After {DEAD}'s death, the elders of {F} chose {N} as the most capable leader."},
    {"title": "The mantle passes to {N}", "desc": "{N} took up the mantle of leadership in {F} after the unexpected death of {DEAD}."},
    {"title": "{F} crowns {N}", "desc": "The people of {F} gathered to witness the crowning of {N}, successor to the fallen {DEAD}."},
    {"title": "{N} assumes command", "desc": "In a brief ceremony, {N} assumed command of {F} following {DEAD}'s demise."},
    {"title": "The torch passes in {F}", "desc": "The torch of leadership in {F} passed from {DEAD} to {N}, who vowed to continue the work."},
    {"title": "{N} takes the scepter", "desc": "With heavy heart and steady hand, {N} took the scepter of {F} from the cold grasp of {DEAD}."},
    {"title": "A new era begins in {F}", "desc": "The era of {DEAD} ended and the era of {N} began in {F}. The future was uncertain."},
    {"title": "{N} proclaimed ruler of {F}", "desc": "The heralds proclaimed {N} as the new ruler of {F}, ending the mourning period for {DEAD}."},
    {"title": "Succession crisis averted in {F}", "desc": "Though {DEAD}'s death threatened chaos, {N} stepped forward and was accepted as {F}'s new leader."},
    {"title": "{N} fills the void in {F}", "desc": "The void left by {DEAD}'s death in {F} was filled by {N}, who rose to the occasion."},
    {"title": "The legacy of {DEAD} continues", "desc": "{N} vowed to continue {DEAD}'s legacy as they assumed control of {F}."}
]

# Write the file
output_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "data", "defaults", "backstory.json")
output_path = os.path.normpath(output_path)
with open(output_path, "w") as f:
    json.dump(data, f, indent=2, ensure_ascii=False)

# Print stats
print(f"Written to: {output_path}")
print(f"Common epithets: {len(data['common_epithets'])}")
print(f"Race epithets: {sum(len(v) for v in data['race_epithets'].values())} across {len(data['race_epithets'])} races")
print(f"Ruler titles: {sum(len(v) for v in data['ruler_titles'].values())} across {len(data['ruler_titles'])} races")
print(f"Dynasty patterns: {sum(len(v) for v in data['dynasty_patterns'].values())} across {len(data['dynasty_patterns'])} races")
print(f"Coronation founding: {sum(len(v) for v in data['coronation_founding'].values())} across {len(data['coronation_founding'])} races")
print(f"Coronation succession: {len(data['coronation_succession'])}")
deaths = data["death_templates"]
total_death = 0
for k, v in deaths.items():
    if isinstance(v, list):
        total_death += len(v)
    elif isinstance(v, dict) and "templates" in v:
        total_death += len(v["templates"])
        print(f"  Diseases: {len(v['diseases'])}")
print(f"Death templates: {total_death} across {len(deaths)} causes")
print(f"Reign event templates: {len(data['reign_event_templates'])}")
print(f"Enemy names: {sum(len(v) for v in data['enemy_names'].values())} across {len(data['enemy_names'])} races")
print(f"Faction adjectives: {len(data['faction_adjectives'])}")
print(f"Plague names: {len(data['plague_names'])}")
print(f"Beast names: {len(data['beast_names'])}")
print(f"Succession templates: {len(data['succession_templates'])}")

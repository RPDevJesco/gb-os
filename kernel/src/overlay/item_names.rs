//! Pokemon Item Name Lookup Tables
//!
//! Contains item names for Gen 1 (Red/Blue/Yellow) and Gen 2 (Gold/Silver/Crystal).

/// Maximum item name length
pub const MAX_ITEM_NAME_LEN: usize = 12;

// =============================================================================
// Gen 1 Item Names (Red/Blue/Yellow)
// =============================================================================

/// Get Gen 1 item name by item ID
pub fn get_gen1_item_name(item_id: u8) -> &'static str {
    if item_id == 0 || (item_id as usize) > GEN1_ITEM_NAMES.len() {
        return "--------";
    }
    GEN1_ITEM_NAMES[item_id as usize - 1]
}

/// Gen 1 item names (83 items)
static GEN1_ITEM_NAMES: [&str; 83] = [
    "MASTER BALL",    // 01
    "ULTRA BALL",     // 02
    "GREAT BALL",     // 03
    "POKE BALL",      // 04
    "TOWN MAP",       // 05
    "BICYCLE",        // 06
    "?????",          // 07 (SURFBOARD in Japanese)
    "SAFARI BALL",    // 08
    "POKEDEX",        // 09
    "MOON STONE",     // 0A
    "ANTIDOTE",       // 0B
    "BURN HEAL",      // 0C
    "ICE HEAL",       // 0D
    "AWAKENING",      // 0E
    "PARLYZ HEAL",    // 0F
    "FULL RESTORE",   // 10
    "MAX POTION",     // 11
    "HYPER POTION",   // 12
    "SUPER POTION",   // 13
    "POTION",         // 14
    "BOULDERBADGE",   // 15
    "CASCADEBADGE",   // 16
    "THUNDERBADGE",   // 17
    "RAINBOWBADGE",   // 18
    "SOULBADGE",      // 19
    "MARSHBADGE",     // 1A
    "VOLCANOBADGE",   // 1B
    "EARTHBADGE",     // 1C
    "ESCAPE ROPE",    // 1D
    "REPEL",          // 1E
    "OLD AMBER",      // 1F
    "FIRE STONE",     // 20
    "THUNDERSTONE",   // 21
    "WATER STONE",    // 22
    "HP UP",          // 23
    "PROTEIN",        // 24
    "IRON",           // 25
    "CARBOS",         // 26
    "CALCIUM",        // 27
    "RARE CANDY",     // 28
    "DOME FOSSIL",    // 29
    "HELIX FOSSIL",   // 2A
    "SECRET KEY",     // 2B
    "?????",          // 2C
    "BIKE VOUCHER",   // 2D
    "X ACCURACY",     // 2E
    "LEAF STONE",     // 2F
    "CARD KEY",       // 30
    "NUGGET",         // 31
    "PP UP",          // 32
    "POKE DOLL",      // 33
    "FULL HEAL",      // 34
    "REVIVE",         // 35
    "MAX REVIVE",     // 36
    "GUARD SPEC.",    // 37
    "SUPER REPEL",    // 38
    "MAX REPEL",      // 39
    "DIRE HIT",       // 3A
    "COIN",           // 3B
    "FRESH WATER",    // 3C
    "SODA POP",       // 3D
    "LEMONADE",       // 3E
    "S.S.TICKET",     // 3F
    "GOLD TEETH",     // 40
    "X ATTACK",       // 41
    "X DEFEND",       // 42
    "X SPEED",        // 43
    "X SPECIAL",      // 44
    "COIN CASE",      // 45
    "OAKS PARCEL",    // 46
    "ITEMFINDER",     // 47
    "SILPH SCOPE",    // 48
    "POKE FLUTE",     // 49
    "LIFT KEY",       // 4A
    "EXP.ALL",        // 4B
    "OLD ROD",        // 4C
    "GOOD ROD",       // 4D
    "SUPER ROD",      // 4E
    "PP UP",          // 4F (duplicate)
    "ETHER",          // 50
    "MAX ETHER",      // 51
    "ELIXER",         // 52
    "MAX ELIXER",     // 53
];

// =============================================================================
// Gen 2 Item Names (Gold/Silver/Crystal)
// =============================================================================

/// Get Gen 2 item name by item ID
pub fn get_gen2_item_name(item_id: u8) -> &'static str {
    if item_id == 0 || (item_id as usize) > GEN2_ITEM_NAMES.len() {
        return "--------";
    }
    GEN2_ITEM_NAMES[item_id as usize - 1]
}

/// Gen 2 item names (255 items, IDs 1-255)
static GEN2_ITEM_NAMES: [&str; 255] = [
    "MASTER BALL",    // 01
    "ULTRA BALL",     // 02
    "BRIGHTPOWDER",   // 03
    "GREAT BALL",     // 04
    "POKE BALL",      // 05
    "TERU-SAMA",      // 06 (unused)
    "BICYCLE",        // 07
    "MOON STONE",     // 08
    "ANTIDOTE",       // 09
    "BURN HEAL",      // 0A
    "ICE HEAL",       // 0B
    "AWAKENING",      // 0C
    "PARLYZ HEAL",    // 0D
    "FULL RESTORE",   // 0E
    "MAX POTION",     // 0F
    "HYPER POTION",   // 10
    "SUPER POTION",   // 11
    "POTION",         // 12
    "ESCAPE ROPE",    // 13
    "REPEL",          // 14
    "MAX ELIXER",     // 15
    "FIRE STONE",     // 16
    "THUNDERSTONE",   // 17
    "WATER STONE",    // 18
    "TERU-SAMA",      // 19
    "HP UP",          // 1A
    "PROTEIN",        // 1B
    "IRON",           // 1C
    "CARBOS",         // 1D
    "LUCKY PUNCH",    // 1E
    "CALCIUM",        // 1F
    "RARE CANDY",     // 20
    "X ACCURACY",     // 21
    "LEAF STONE",     // 22
    "METAL POWDER",   // 23
    "NUGGET",         // 24
    "POKE DOLL",      // 25
    "FULL HEAL",      // 26
    "REVIVE",         // 27
    "MAX REVIVE",     // 28
    "GUARD SPEC.",    // 29
    "SUPER REPEL",    // 2A
    "MAX REPEL",      // 2B
    "DIRE HIT",       // 2C
    "TERU-SAMA",      // 2D
    "FRESH WATER",    // 2E
    "SODA POP",       // 2F
    "LEMONADE",       // 30
    "X ATTACK",       // 31
    "TERU-SAMA",      // 32
    "X DEFEND",       // 33
    "X SPEED",        // 34
    "X SPECIAL",      // 35
    "COIN CASE",      // 36
    "ITEMFINDER",     // 37
    "TERU-SAMA",      // 38
    "EXP.SHARE",      // 39
    "OLD ROD",        // 3A
    "GOOD ROD",       // 3B
    "SILVER LEAF",    // 3C
    "SUPER ROD",      // 3D
    "PP UP",          // 3E
    "ETHER",          // 3F
    "MAX ETHER",      // 40
    "ELIXER",         // 41
    "RED SCALE",      // 42
    "SECRETPOTION",   // 43
    "S.S.TICKET",     // 44
    "MYSTERY EGG",    // 45
    "CLEAR BELL",     // 46
    "SILVER WING",    // 47
    "MOOMOO MILK",    // 48
    "QUICK CLAW",     // 49
    "PSNCUREBERRY",   // 4A
    "GOLD LEAF",      // 4B
    "SOFT SAND",      // 4C
    "SHARP BEAK",     // 4D
    "PRZCUREBERRY",   // 4E
    "BURNT BERRY",    // 4F
    "ICE BERRY",      // 50
    "POISON BARB",    // 51
    "KINGS ROCK",     // 52
    "BITTER BERRY",   // 53
    "MINT BERRY",     // 54
    "RED APRICORN",   // 55
    "TINYMUSHROOM",   // 56
    "BIG MUSHROOM",   // 57
    "SILVERPOWDER",   // 58
    "BLU APRICORN",   // 59
    "TERU-SAMA",      // 5A
    "AMULET COIN",    // 5B
    "YLW APRICORN",   // 5C
    "GRN APRICORN",   // 5D
    "CLEANSE TAG",    // 5E
    "MYSTIC WATER",   // 5F
    "TWISTEDSPOON",   // 60
    "WHT APRICORN",   // 61
    "BLACKBELT",      // 62
    "BLK APRICORN",   // 63
    "TERU-SAMA",      // 64
    "PNK APRICORN",   // 65
    "BLACKGLASSES",   // 66
    "SLOWPOKETAIL",   // 67
    "PINK BOW",       // 68
    "STICK",          // 69
    "SMOKE BALL",     // 6A
    "NEVERMELTICE",   // 6B
    "MAGNET",         // 6C
    "MIRACLEBERRY",   // 6D
    "PEARL",          // 6E
    "BIG PEARL",      // 6F
    "EVERSTONE",      // 70
    "SPELL TAG",      // 71
    "RAGECANDYBAR",   // 72
    "GS BALL",        // 73
    "BLUE CARD",      // 74
    "MIRACLE SEED",   // 75
    "THICK CLUB",     // 76
    "FOCUS BAND",     // 77
    "TERU-SAMA",      // 78
    "ENERGYPOWDER",   // 79
    "ENERGY ROOT",    // 7A
    "HEAL POWDER",    // 7B
    "REVIVAL HERB",   // 7C
    "HARD STONE",     // 7D
    "LUCKY EGG",      // 7E
    "CARD KEY",       // 7F
    "MACHINE PART",   // 80
    "EGG TICKET",     // 81
    "LOST ITEM",      // 82
    "STARDUST",       // 83
    "STAR PIECE",     // 84
    "BASEMENT KEY",   // 85
    "PASS",           // 86
    "TERU-SAMA",      // 87
    "TERU-SAMA",      // 88
    "TERU-SAMA",      // 89
    "CHARCOAL",       // 8A
    "BERRY JUICE",    // 8B
    "SCOPE LENS",     // 8C
    "TERU-SAMA",      // 8D
    "TERU-SAMA",      // 8E
    "METAL COAT",     // 8F
    "DRAGON FANG",    // 90
    "TERU-SAMA",      // 91
    "LEFTOVERS",      // 92
    "TERU-SAMA",      // 93
    "TERU-SAMA",      // 94
    "TERU-SAMA",      // 95
    "MYSTERYBERRY",   // 96
    "DRAGON SCALE",   // 97
    "BERSERK GENE",   // 98
    "TERU-SAMA",      // 99
    "TERU-SAMA",      // 9A
    "TERU-SAMA",      // 9B
    "SACRED ASH",     // 9C
    "HEAVY BALL",     // 9D
    "FLOWER MAIL",    // 9E
    "LEVEL BALL",     // 9F
    "LURE BALL",      // A0
    "FAST BALL",      // A1
    "TERU-SAMA",      // A2
    "LIGHT BALL",     // A3
    "FRIEND BALL",    // A4
    "MOON BALL",      // A5
    "LOVE BALL",      // A6
    "NORMAL BOX",     // A7
    "GORGEOUS BOX",   // A8
    "SUN STONE",      // A9
    "POLKADOT BOW",   // AA
    "TERU-SAMA",      // AB
    "UP-GRADE",       // AC
    "BERRY",          // AD
    "GOLD BERRY",     // AE
    "SQUIRTBOTTLE",   // AF
    "TERU-SAMA",      // B0
    "PARK BALL",      // B1
    "RAINBOW WING",   // B2
    "TERU-SAMA",      // B3
    "BRICK PIECE",    // B4
    "SURF MAIL",      // B5
    "LITEBLUEMAIL",   // B6
    "PORTRAITMAIL",   // B7
    "LOVELY MAIL",    // B8
    "EON MAIL",       // B9
    "MORPH MAIL",     // BA
    "BLUESKY MAIL",   // BB
    "MUSIC MAIL",     // BC
    "MIRAGE MAIL",    // BD
    "TERU-SAMA",      // BE
    "TM01",           // BF
    "TM02",           // C0
    "TM03",           // C1
    "TM04",           // C2
    "TM05",           // C3
    "TM06",           // C4
    "TM07",           // C5
    "TM08",           // C6
    "TM09",           // C7
    "TM10",           // C8
    "TM11",           // C9
    "TM12",           // CA
    "TM13",           // CB
    "TM14",           // CC
    "TM15",           // CD
    "TM16",           // CE
    "TM17",           // CF
    "TM18",           // D0
    "TM19",           // D1
    "TM20",           // D2
    "TM21",           // D3
    "TM22",           // D4
    "TM23",           // D5
    "TM24",           // D6
    "TM25",           // D7
    "TM26",           // D8
    "TM27",           // D9
    "TM28",           // DA
    "TM29",           // DB
    "TM30",           // DC
    "TM31",           // DD
    "TM32",           // DE
    "TM33",           // DF
    "TM34",           // E0
    "TM35",           // E1
    "TM36",           // E2
    "TM37",           // E3
    "TM38",           // E4
    "TM39",           // E5
    "TM40",           // E6
    "TM41",           // E7
    "TM42",           // E8
    "TM43",           // E9
    "TM44",           // EA
    "TM45",           // EB
    "TM46",           // EC
    "TM47",           // ED
    "TM48",           // EE
    "TM49",           // EF
    "TM50",           // F0
    "HM01",           // F1
    "HM02",           // F2
    "HM03",           // F3
    "HM04",           // F4
    "HM05",           // F5
    "HM06",           // F6
    "HM07",           // F7
    "TERU-SAMA",      // F8
    "TERU-SAMA",      // F9
    "TERU-SAMA",      // FA
    "TERU-SAMA",      // FB
    "TERU-SAMA",      // FC
    "TERU-SAMA",      // FD
    "TERU-SAMA",      // FE
    "CANCEL",         // FF
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen1_items() {
        assert_eq!(get_gen1_item_name(1), "MASTER BALL");
        assert_eq!(get_gen1_item_name(4), "POKE BALL");
        assert_eq!(get_gen1_item_name(0), "--------");
    }

    #[test]
    fn test_gen2_items() {
        assert_eq!(get_gen2_item_name(1), "MASTER BALL");
        assert_eq!(get_gen2_item_name(5), "POKE BALL");
        assert_eq!(get_gen2_item_name(0), "--------");
    }
}

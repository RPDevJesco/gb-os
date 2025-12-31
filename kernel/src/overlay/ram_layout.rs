//! Pokemon RAM Layout Definitions
//!
//! Memory addresses and structures for reading game state from RAM.
//! Supports all Gen 1 and Gen 2 Pokemon games:
//! - Gen 1: Red, Blue, Yellow (each with unique offsets)
//! - Gen 2: Gold, Silver, Crystal (each with unique offsets)
//!
//! References:
//! - https://github.com/pret/pokered (pokered disassembly)
//! - https://github.com/pret/pokeyellow (pokeyellow disassembly)
//! - https://github.com/pret/pokegold (pokegold disassembly)
//! - https://github.com/pret/pokecrystal (pokecrystal disassembly)
//! - https://datacrystal.romhacking.net/wiki/Pok%C3%A9mon_Red/Blue

use crate::gameboy::mmu::MMU;

// =============================================================================
// Game Detection
// =============================================================================

/// Supported Pokemon games for overlay
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Game {
    Red,
    Blue,
    Yellow,
    Gold,
    Silver,
    Crystal,
    Unknown,
}

impl Game {
    /// Detect game from ROM header title
    /// Call with the ROM name from `device.romname()`
    ///
    /// ROM titles from cartridge headers:
    /// - "POKEMON RED" / "POKEMON BLUE" / "POKEMON YELLOW"
    /// - "POKEMON GOLD" / "POKEMON SILVER" / "POKEMON CRYSTAL" (or "PM_CRYSTAL")
    pub fn detect(rom_name: &str) -> Self {
        let bytes = rom_name.as_bytes();

        // Gen 1 games - check specific versions
        if Self::contains_ignore_case(bytes, b"YELLOW") {
            return Game::Yellow;
        }
        if Self::contains_ignore_case(bytes, b"BLUE") {
            return Game::Blue;
        }
        if Self::contains_ignore_case(bytes, b"RED") {
            return Game::Red;
        }

        // Gen 2 games - check specific versions
        if Self::contains_ignore_case(bytes, b"CRYSTAL") {
            return Game::Crystal;
        }
        if Self::contains_ignore_case(bytes, b"SILVER") {
            return Game::Silver;
        }
        if Self::contains_ignore_case(bytes, b"GOLD") {
            return Game::Gold;
        }

        Game::Unknown
    }

    /// Get the generation (1 or 2, 0 for unknown)
    pub fn generation(&self) -> u8 {
        match self {
            Game::Red | Game::Blue | Game::Yellow => 1,
            Game::Gold | Game::Silver | Game::Crystal => 2,
            Game::Unknown => 0,
        }
    }

    /// Check if this is a Gen 1 game
    pub fn is_gen1(&self) -> bool {
        self.generation() == 1
    }

    /// Check if this is a Gen 2 game
    pub fn is_gen2(&self) -> bool {
        self.generation() == 2
    }

    /// Case-insensitive substring search (no allocation)
    fn contains_ignore_case(haystack: &[u8], needle: &[u8]) -> bool {
        if needle.is_empty() {
            return true;
        }
        if haystack.len() < needle.len() {
            return false;
        }

        'outer: for i in 0..=(haystack.len() - needle.len()) {
            for j in 0..needle.len() {
                let h = haystack[i + j].to_ascii_uppercase();
                let n = needle[j].to_ascii_uppercase();
                if h != n {
                    continue 'outer;
                }
            }
            return true;
        }
        false
    }
}

// =============================================================================
// Pokemon Red/Blue (Gen 1) Addresses
// =============================================================================

pub mod red_blue {
    //! Pokemon Red/Blue RAM addresses
    //! Based on pokered disassembly (WRAM bank 1 starts at 0xD000)

    // --- Player Info ---
    pub const PLAYER_NAME: u16 = 0xD158;        // 11 bytes (name + terminator)
    pub const PLAYER_NAME_LEN: u16 = 11;
    pub const PLAYER_ID: u16 = 0xD359;          // 2 bytes
    pub const RIVAL_NAME: u16 = 0xD34A;         // 11 bytes
    pub const MONEY: u16 = 0xD347;              // 3 bytes BCD
    pub const COINS: u16 = 0xD5A4;              // 2 bytes BCD (Game Corner)
    pub const BADGES: u16 = 0xD356;             // 1 byte (bit flags)

    // Enemy party / trainer
    pub const ENEMY_PARTY_COUNT: u16 = 0xD89C;      // 1 byte (in trainer battles)

    // Bag
    pub const BAG_ITEM_COUNT: u16 = 0xD31D;         // Number of items in bag
    pub const BAG_ITEM_DATA: u16 = 0xD31E;          // Item data (item_id, quantity pairs)

    // --- Play Time ---
    pub const PLAY_TIME_HOURS: u16 = 0xDA40;    // 2 bytes (max 255:59:59)
    pub const PLAY_TIME_MINUTES: u16 = 0xDA42;  // 1 byte
    pub const PLAY_TIME_SECONDS: u16 = 0xDA44;  // 1 byte

    // --- Party Pokemon ---
    pub const PARTY_COUNT: u16 = 0xD163;        // 1 byte (0-6)
    pub const PARTY_SPECIES: u16 = 0xD164;      // 6 bytes + terminator (0xFF)
    pub const PARTY_DATA: u16 = 0xD16B;         // 44 bytes per Pokemon × 6
    pub const PARTY_MON_SIZE: u16 = 44;
    pub const PARTY_OT_NAMES: u16 = 0xD273;     // 11 bytes × 6 (original trainer)
    pub const PARTY_NICKNAMES: u16 = 0xD2B5;    // 11 bytes × 6

    // --- Party Pokemon Structure Offsets (within 44-byte block) ---
    pub const MON_SPECIES: u16 = 0;             // 1 byte
    pub const MON_HP_CURRENT: u16 = 1;          // 2 bytes
    pub const MON_LEVEL_PC: u16 = 3;            // 1 byte (level from box, may differ)
    pub const MON_STATUS: u16 = 4;              // 1 byte (status condition)
    pub const MON_TYPE1: u16 = 5;               // 1 byte
    pub const MON_TYPE2: u16 = 6;               // 1 byte
    pub const MON_CATCH_RATE: u16 = 7;          // 1 byte (held item in Gen 2)
    pub const MON_MOVES: u16 = 8;               // 4 bytes (move indices)
    pub const MON_OT_ID: u16 = 12;              // 2 bytes
    pub const MON_EXP: u16 = 14;                // 3 bytes
    pub const MON_HP_EV: u16 = 17;              // 2 bytes
    pub const MON_ATK_EV: u16 = 19;             // 2 bytes
    pub const MON_DEF_EV: u16 = 21;             // 2 bytes
    pub const MON_SPD_EV: u16 = 23;             // 2 bytes
    pub const MON_SPC_EV: u16 = 25;             // 2 bytes (Special)
    pub const MON_IV: u16 = 27;                 // 2 bytes (packed IVs)
    pub const MON_PP: u16 = 29;                 // 4 bytes (PP for each move)
    pub const MON_LEVEL: u16 = 33;              // 1 byte (actual level)
    pub const MON_HP_MAX: u16 = 34;             // 2 bytes
    pub const MON_ATTACK: u16 = 36;             // 2 bytes
    pub const MON_DEFENSE: u16 = 38;            // 2 bytes
    pub const MON_SPEED: u16 = 40;              // 2 bytes
    pub const MON_SPECIAL: u16 = 42;            // 2 bytes

    // --- Pokedex ---
    pub const POKEDEX_OWNED: u16 = 0xD2F7;      // 19 bytes (bit flags, 151 Pokemon)
    pub const POKEDEX_SEEN: u16 = 0xD30A;       // 19 bytes

    // --- Location ---
    pub const CURRENT_MAP: u16 = 0xD35E;        // 1 byte
    pub const MAP_Y: u16 = 0xD361;              // 1 byte
    pub const MAP_X: u16 = 0xD362;              // 1 byte

    // --- Battle ---
    pub const IN_BATTLE: u16 = 0xD057;          // 1 byte (0 = no, 1 = wild, 2 = trainer)

    // Player's active Pokemon in battle
    pub const BATTLE_YOUR_SPECIES: u16 = 0xD014;  // 1 byte
    pub const BATTLE_YOUR_HP: u16 = 0xD015;       // 2 bytes
    pub const BATTLE_YOUR_STATUS: u16 = 0xD018;   // 1 byte
    pub const BATTLE_YOUR_MOVES: u16 = 0xD01C;    // 4 bytes
    pub const BATTLE_YOUR_LEVEL: u16 = 0xD022;    // 1 byte
    pub const BATTLE_YOUR_HP_MAX: u16 = 0xD023;   // 2 bytes
    pub const BATTLE_YOUR_ATTACK: u16 = 0xD025;   // 2 bytes
    pub const BATTLE_YOUR_DEFENSE: u16 = 0xD027;  // 2 bytes
    pub const BATTLE_YOUR_SPEED: u16 = 0xD029;    // 2 bytes
    pub const BATTLE_YOUR_SPECIAL: u16 = 0xD02B;  // 2 bytes
    pub const BATTLE_YOUR_PP: u16 = 0xD02D;       // 4 bytes

    // Enemy Pokemon in battle
    pub const ENEMY_MON_SPECIES: u16 = 0xCFD8;    // 1 byte
    pub const ENEMY_MON_HP: u16 = 0xCFE6;         // 2 bytes
    pub const ENEMY_MON_LEVEL: u16 = 0xCFF3;      // 1 byte
    pub const ENEMY_MON_HP_MAX: u16 = 0xCFF4;     // 2 bytes
    pub const ENEMY_MON_MOVES: u16 = 0xCFED;      // 4 bytes
    pub const ENEMY_MON_ATTACK: u16 = 0xCFF6;     // 2 bytes
    pub const ENEMY_MON_DEFENSE: u16 = 0xCFF8;    // 2 bytes
    pub const ENEMY_MON_SPEED: u16 = 0xCFFA;      // 2 bytes
    pub const ENEMY_MON_SPECIAL: u16 = 0xCFFC;    // 2 bytes

    // --- Text Encoding ---
    pub const CHAR_TERMINATOR: u8 = 0x50;
    pub const CHAR_SPACE: u8 = 0x7F;
}

// =============================================================================
// Pokemon Yellow (Gen 1) Addresses
// =============================================================================

pub mod yellow {
    //! Pokemon Yellow RAM addresses
    //! Based on pokeyellow disassembly (WRAM bank 1 starts at 0xD000)
    //! Note: Most addresses are 1 byte lower than Red/Blue

    // --- Player Info ---
    pub const PLAYER_NAME: u16 = 0xD157;        // 11 bytes (name + terminator)
    pub const PLAYER_NAME_LEN: u16 = 11;
    pub const PLAYER_ID: u16 = 0xD358;          // 2 bytes
    pub const RIVAL_NAME: u16 = 0xD349;         // 11 bytes
    pub const MONEY: u16 = 0xD346;              // 3 bytes BCD
    pub const COINS: u16 = 0xD5A3;              // 2 bytes BCD (Game Corner)
    pub const BADGES: u16 = 0xD355;             // 1 byte (bit flags)

    // --- Play Time ---
    pub const PLAY_TIME_HOURS: u16 = 0xDA3F;    // 2 bytes (max 255:59:59)
    pub const PLAY_TIME_MINUTES: u16 = 0xDA41;  // 1 byte
    pub const PLAY_TIME_SECONDS: u16 = 0xDA43;  // 1 byte
    pub const PLAY_TIME_FRAMES: u16 = 0xDA44;   // 1 byte (60 fps)

    // --- Party Pokemon ---
    pub const PARTY_COUNT: u16 = 0xD162;        // 1 byte (0-6)
    pub const PARTY_SPECIES: u16 = 0xD163;      // 6 bytes + terminator (0xFF)
    pub const PARTY_DATA: u16 = 0xD16A;         // 44 bytes per Pokemon × 6
    pub const PARTY_MON_SIZE: u16 = 44;
    pub const PARTY_OT_NAMES: u16 = 0xD272;     // 11 bytes × 6 (original trainer)
    pub const PARTY_NICKNAMES: u16 = 0xD2B4;    // 11 bytes × 6

    // --- Party Pokemon Structure Offsets (same as Red/Blue) ---
    pub const MON_SPECIES: u16 = 0;
    pub const MON_HP_CURRENT: u16 = 1;
    pub const MON_LEVEL_PC: u16 = 3;
    pub const MON_STATUS: u16 = 4;
    pub const MON_TYPE1: u16 = 5;
    pub const MON_TYPE2: u16 = 6;
    pub const MON_CATCH_RATE: u16 = 7;
    pub const MON_MOVES: u16 = 8;
    pub const MON_OT_ID: u16 = 12;
    pub const MON_EXP: u16 = 14;
    pub const MON_HP_EV: u16 = 17;
    pub const MON_ATK_EV: u16 = 19;
    pub const MON_DEF_EV: u16 = 21;
    pub const MON_SPD_EV: u16 = 23;
    pub const MON_SPC_EV: u16 = 25;
    pub const MON_IV: u16 = 27;
    pub const MON_PP: u16 = 29;
    pub const MON_LEVEL: u16 = 33;
    pub const MON_HP_MAX: u16 = 34;
    pub const MON_ATTACK: u16 = 36;
    pub const MON_DEFENSE: u16 = 38;
    pub const MON_SPEED: u16 = 40;
    pub const MON_SPECIAL: u16 = 42;

    // Enemy party / trainer
    pub const ENEMY_PARTY_COUNT: u16 = 0xD89B;      // 1 byte (in trainer battles)

    // Bag
    pub const BAG_ITEM_COUNT: u16 = 0xD31C;         // Number of items in bag
    pub const BAG_ITEM_DATA: u16 = 0xD31D;          // Item data (item_id, quantity pairs)

    // --- Pokedex ---
    pub const POKEDEX_OWNED: u16 = 0xD2F6;      // 19 bytes (bit flags, 151 Pokemon)
    pub const POKEDEX_SEEN: u16 = 0xD309;       // 19 bytes

    // --- Location ---
    pub const CURRENT_MAP: u16 = 0xD35D;        // 1 byte
    pub const MAP_Y: u16 = 0xD360;              // 1 byte
    pub const MAP_X: u16 = 0xD361;              // 1 byte

    // --- Battle ---
    pub const IN_BATTLE: u16 = 0xD057;          // 1 byte (0 = no, 1 = wild, 2 = trainer)

    // Player's active Pokemon in battle (same as Red/Blue)
    pub const BATTLE_YOUR_SPECIES: u16 = 0xD014;
    pub const BATTLE_YOUR_HP: u16 = 0xD015;
    pub const BATTLE_YOUR_STATUS: u16 = 0xD018;
    pub const BATTLE_YOUR_MOVES: u16 = 0xD01C;
    pub const BATTLE_YOUR_LEVEL: u16 = 0xD022;
    pub const BATTLE_YOUR_HP_MAX: u16 = 0xD023;
    pub const BATTLE_YOUR_ATTACK: u16 = 0xD025;
    pub const BATTLE_YOUR_DEFENSE: u16 = 0xD027;
    pub const BATTLE_YOUR_SPEED: u16 = 0xD029;
    pub const BATTLE_YOUR_SPECIAL: u16 = 0xD02B;
    pub const BATTLE_YOUR_PP: u16 = 0xD02D;

    // Enemy Pokemon in battle
    pub const ENEMY_MON_SPECIES: u16 = 0xCFD8;
    pub const ENEMY_MON_HP: u16 = 0xCFE6;
    pub const ENEMY_MON_LEVEL: u16 = 0xCFF3;
    pub const ENEMY_MON_HP_MAX: u16 = 0xCFF4;
    pub const ENEMY_MON_MOVES: u16 = 0xCFED;
    pub const ENEMY_MON_ATTACK: u16 = 0xCFF6;
    pub const ENEMY_MON_DEFENSE: u16 = 0xCFF8;
    pub const ENEMY_MON_SPEED: u16 = 0xCFFA;
    pub const ENEMY_MON_SPECIAL: u16 = 0xCFFC;

    // --- Pikachu (Yellow-specific) ---
    pub const PIKACHU_HAPPINESS: u16 = 0xD46F;  // 1 byte (0-255)
    pub const PIKACHU_MAP_Y: u16 = 0xC452;      // Pikachu's Y position
    pub const PIKACHU_MAP_X: u16 = 0xC453;      // Pikachu's X position

    // --- Text Encoding ---
    pub const CHAR_TERMINATOR: u8 = 0x50;
    pub const CHAR_SPACE: u8 = 0x7F;
}

// =============================================================================
// Pokemon Gold/Silver (Gen 2) Addresses
// =============================================================================

pub mod gold_silver {
    //! Pokemon Gold/Silver RAM addresses
    //! Based on pokegold disassembly

    // --- Player Info ---
    pub const PLAYER_NAME: u16 = 0xD1A3;        // 11 bytes
    pub const PLAYER_NAME_LEN: u16 = 11;
    pub const PLAYER_ID: u16 = 0xD1A1;          // 2 bytes
    pub const MONEY: u16 = 0xD573;              // 3 bytes
    pub const MOM_MONEY: u16 = 0xD576;          // 3 bytes
    pub const COINS: u16 = 0xD57A;              // 2 bytes (Game Corner)
    pub const JOHTO_BADGES: u16 = 0xD57C;       // 1 byte (bit flags)
    pub const KANTO_BADGES: u16 = 0xD57D;       // 1 byte (bit flags)

    // Enemy party
    pub const ENEMY_PARTY_COUNT: u16 = 0xD0D4;      // 1 byte
    pub const ENEMY_PARTY_SPECIES: u16 = 0xD0D5;    // 6 bytes

    // Battle type (0=no battle, 1=wild, 2=trainer)
    pub const BATTLE_MODE: u16 = 0xD116;            // 1 byte
    pub const BATTLE_TYPE: u16 = 0xD117;            // 1 byte (right after BATTLE_MODE)

    // Bag items (Gen 2 has multiple pockets)
    pub const ITEMS_POCKET_COUNT: u16 = 0xD5B7;     // Number of items in Items pocket
    pub const ITEMS_POCKET_DATA: u16 = 0xD5B8;      // Items pocket data (item_id, quantity pairs)
    pub const KEY_ITEMS_COUNT: u16 = 0xD5E1;        // Number of key items
    pub const KEY_ITEMS_DATA: u16 = 0xD5E2;         // Key items data
    pub const BALLS_POCKET_COUNT: u16 = 0xD5FC;     // Number of ball types
    pub const BALLS_POCKET_DATA: u16 = 0xD5FD;      // Balls pocket data

    // --- Play Time ---
    pub const PLAY_TIME_HOURS: u16 = 0xD1EB;    // 2 bytes
    pub const PLAY_TIME_MINUTES: u16 = 0xD1ED;  // 1 byte
    pub const PLAY_TIME_SECONDS: u16 = 0xD1EE;  // 1 byte

    // --- Party Pokemon ---
    pub const PARTY_COUNT: u16 = 0xDA22;        // 1 byte (0-6)
    pub const PARTY_SPECIES: u16 = 0xDA23;      // 6 bytes + terminator
    pub const PARTY_DATA: u16 = 0xDA2A;         // 48 bytes per Pokemon × 6
    pub const PARTY_MON_SIZE: u16 = 48;
    pub const PARTY_OT_NAMES: u16 = 0xDB4A;     // 11 bytes × 6
    pub const PARTY_NICKNAMES: u16 = 0xDB8C;    // 11 bytes × 6

    // --- Party Pokemon Structure Offsets (within 48-byte block) ---
    pub const MON_SPECIES: u16 = 0;             // 1 byte
    pub const MON_ITEM: u16 = 1;                // 1 byte (held item)
    pub const MON_MOVES: u16 = 2;               // 4 bytes
    pub const MON_OT_ID: u16 = 6;               // 2 bytes
    pub const MON_EXP: u16 = 8;                 // 3 bytes
    pub const MON_HP_EV: u16 = 11;              // 2 bytes
    pub const MON_ATK_EV: u16 = 13;             // 2 bytes
    pub const MON_DEF_EV: u16 = 15;             // 2 bytes
    pub const MON_SPD_EV: u16 = 17;             // 2 bytes
    pub const MON_SPC_EV: u16 = 19;             // 2 bytes
    pub const MON_IV: u16 = 21;                 // 2 bytes (packed)
    pub const MON_PP: u16 = 23;                 // 4 bytes
    pub const MON_FRIENDSHIP: u16 = 27;         // 1 byte
    pub const MON_POKERUS: u16 = 28;            // 1 byte
    pub const MON_CAUGHT_DATA: u16 = 29;        // 2 bytes (time/location)
    pub const MON_LEVEL: u16 = 31;              // 1 byte
    pub const MON_STATUS: u16 = 32;             // 1 byte
    pub const MON_HP_CURRENT: u16 = 34;         // 2 bytes
    pub const MON_HP_MAX: u16 = 36;             // 2 bytes
    pub const MON_ATTACK: u16 = 38;             // 2 bytes
    pub const MON_DEFENSE: u16 = 40;            // 2 bytes
    pub const MON_SPEED: u16 = 42;              // 2 bytes
    pub const MON_SP_ATK: u16 = 44;             // 2 bytes (Special Attack)
    pub const MON_SP_DEF: u16 = 46;             // 2 bytes (Special Defense)

    // --- Pokedex ---
    pub const POKEDEX_OWNED: u16 = 0xDE99;      // 32 bytes (bit flags, 251 Pokemon)
    pub const POKEDEX_SEEN: u16 = 0xDEB9;       // 32 bytes

    // --- Location ---
    pub const MAP_GROUP: u16 = 0xDA00;          // 1 byte
    pub const MAP_NUMBER: u16 = 0xDA01;         // 1 byte
    pub const MAP_Y: u16 = 0xDA02;              // 1 byte
    pub const MAP_X: u16 = 0xDA03;              // 1 byte

    // --- Time ---
    pub const TIME_HOURS: u16 = 0xD4B4;         // 1 byte (0-23)
    pub const TIME_MINUTES: u16 = 0xD4B5;       // 1 byte
    pub const TIME_SECONDS: u16 = 0xD4B6;       // 1 byte
    pub const TIME_DAY: u16 = 0xD4B3;           // 1 byte (day of week)

    // Player's active Pokemon in battle
    pub const BATTLE_YOUR_SPECIES: u16 = 0xCB0C;  // 1 byte
    pub const BATTLE_YOUR_ITEM: u16 = 0xCB0D;     // 1 byte
    pub const BATTLE_YOUR_MOVES: u16 = 0xCB0E;    // 4 bytes
    pub const BATTLE_YOUR_PP: u16 = 0xCB14;       // 4 bytes
    pub const BATTLE_YOUR_HP: u16 = 0xCB1C;       // 2 bytes
    pub const BATTLE_YOUR_HP_MAX: u16 = 0xCBBF;   // 2 bytes
    pub const BATTLE_YOUR_ATTACK: u16 = 0xCBC1;   // 2 bytes
    pub const BATTLE_YOUR_DEFENSE: u16 = 0xCBC3;  // 2 bytes
    pub const BATTLE_YOUR_SPEED: u16 = 0xCBC5;    // 2 bytes
    pub const BATTLE_YOUR_SP_ATK: u16 = 0xCBC7;   // 2 bytes
    pub const BATTLE_YOUR_SP_DEF: u16 = 0xCBC9;   // 2 bytes
    pub const BATTLE_YOUR_LEVEL: u16 = 0xCB1E;    // 1 byte (estimated)

    // Enemy Pokemon in battle
    pub const ENEMY_MON_SPECIES: u16 = 0xD0EF;    // 1 byte
    pub const ENEMY_MON_ITEM: u16 = 0xD0F0;       // 1 byte
    pub const ENEMY_MON_MOVES: u16 = 0xD0F1;      // 4 bytes
    pub const ENEMY_MON_PP: u16 = 0xD0F7;         // 4 bytes
    pub const ENEMY_MON_LEVEL: u16 = 0xD0FC;      // 1 byte
    pub const ENEMY_MON_HP: u16 = 0xD0FF;         // 2 bytes
    pub const ENEMY_MON_HP_MAX: u16 = 0xD101;     // 2 bytes
    pub const ENEMY_MON_ATTACK: u16 = 0xD103;     // 2 bytes
    pub const ENEMY_MON_DEFENSE: u16 = 0xD105;    // 2 bytes
    pub const ENEMY_MON_SPEED: u16 = 0xD107;      // 2 bytes
    pub const ENEMY_MON_SP_ATK: u16 = 0xD109;     // 2 bytes
    pub const ENEMY_MON_SP_DEF: u16 = 0xD10B;     // 2 bytes

    // --- Text Encoding (Gen 2) ---
    pub const CHAR_TERMINATOR: u8 = 0x50;
    pub const CHAR_SPACE: u8 = 0x7F;
}

// =============================================================================
// Pokemon Crystal (Gen 2) Addresses
// =============================================================================

pub mod crystal {
    //! Pokemon Crystal RAM addresses
    //! Based on pokecrystal disassembly
    //! Note: Significantly different from Gold/Silver

    // --- Player Info ---
    pub const PLAYER_NAME: u16 = 0xD47D;        // 11 bytes
    pub const PLAYER_NAME_LEN: u16 = 11;
    pub const PLAYER_ID: u16 = 0xD47B;          // 2 bytes
    pub const PLAYER_GENDER: u16 = 0xD472;      // 1 byte (0 = male, 1 = female)
    pub const MONEY: u16 = 0xD84E;              // 3 bytes BCD
    pub const MOM_MONEY: u16 = 0xD851;          // 3 bytes
    pub const COINS: u16 = 0xD855;              // 2 bytes BCD
    pub const JOHTO_BADGES: u16 = 0xD857;       // 1 byte (bit flags)
    pub const KANTO_BADGES: u16 = 0xD858;       // 1 byte (bit flags)

    // --- Play Time ---
    pub const PLAY_TIME_HOURS: u16 = 0xD4C4;    // 2 bytes
    pub const PLAY_TIME_MINUTES: u16 = 0xD4C6;  // 1 byte
    pub const PLAY_TIME_SECONDS: u16 = 0xD4C7;  // 1 byte

    // --- Party Pokemon ---
    pub const PARTY_COUNT: u16 = 0xDCD7;        // 1 byte (0-6)
    pub const PARTY_SPECIES: u16 = 0xDCD8;      // 6 bytes + terminator
    pub const PARTY_DATA: u16 = 0xDCDF;         // 48 bytes per Pokemon × 6
    pub const PARTY_MON_SIZE: u16 = 48;
    pub const PARTY_OT_NAMES: u16 = 0xDDFF;     // 11 bytes × 6
    pub const PARTY_NICKNAMES: u16 = 0xDE41;    // 11 bytes × 6

    // Enemy party / trainer
    pub const ENEMY_PARTY_COUNT: u16 = 0xD89C;      // 1 byte (in trainer battles)
    pub const ENEMY_PARTY_SPECIES: u16 = 0xD0D5;    // 6 bytes

    // Bag
    pub const BAG_ITEM_COUNT: u16 = 0xD31D;         // Number of items in bag
    pub const BAG_ITEM_DATA: u16 = 0xD31E;          // Item data (item_id, quantity pairs)
    pub const ITEMS_POCKET_COUNT: u16 = 0xD892;
    pub const ITEMS_POCKET_DATA: u16 = 0xD893;
    pub const KEY_ITEMS_COUNT: u16 = 0xD8BC;
    pub const KEY_ITEMS_DATA: u16 = 0xD8BD;
    pub const BALLS_POCKET_COUNT: u16 = 0xD8D7;
    pub const BALLS_POCKET_DATA: u16 = 0xD8D8;

    // --- Party Pokemon Structure Offsets (same as Gold/Silver) ---
    pub const MON_SPECIES: u16 = 0;
    pub const MON_ITEM: u16 = 1;
    pub const MON_MOVES: u16 = 2;
    pub const MON_OT_ID: u16 = 6;
    pub const MON_EXP: u16 = 8;
    pub const MON_HP_EV: u16 = 11;
    pub const MON_ATK_EV: u16 = 13;
    pub const MON_DEF_EV: u16 = 15;
    pub const MON_SPD_EV: u16 = 17;
    pub const MON_SPC_EV: u16 = 19;
    pub const MON_IV: u16 = 21;
    pub const MON_PP: u16 = 23;
    pub const MON_FRIENDSHIP: u16 = 27;
    pub const MON_POKERUS: u16 = 28;
    pub const MON_CAUGHT_DATA: u16 = 29;
    pub const MON_LEVEL: u16 = 31;
    pub const MON_STATUS: u16 = 32;
    pub const MON_HP_CURRENT: u16 = 34;
    pub const MON_HP_MAX: u16 = 36;
    pub const MON_ATTACK: u16 = 38;
    pub const MON_DEFENSE: u16 = 40;
    pub const MON_SPEED: u16 = 42;
    pub const MON_SP_ATK: u16 = 44;
    pub const MON_SP_DEF: u16 = 46;

    // --- Pokedex ---
    pub const POKEDEX_OWNED: u16 = 0xDE99;      // 32 bytes (bit flags, 251 Pokemon)
    pub const POKEDEX_SEEN: u16 = 0xDEB9;       // 32 bytes

    // --- Location ---
    pub const MAP_GROUP: u16 = 0xDCB5;          // 1 byte
    pub const MAP_NUMBER: u16 = 0xDCB6;         // 1 byte
    pub const MAP_Y: u16 = 0xDCB7;              // 1 byte
    pub const MAP_X: u16 = 0xDCB8;              // 1 byte

    // --- Time ---
    pub const TIME_HOURS: u16 = 0xD4B4;         // 1 byte (0-23)
    pub const TIME_MINUTES: u16 = 0xD4B5;       // 1 byte
    pub const TIME_SECONDS: u16 = 0xD4B6;       // 1 byte
    pub const TIME_DAY: u16 = 0xD4B3;           // 1 byte (day of week)

    // --- Battle ---
    pub const BATTLE_MODE: u16 = 0xD22D;        // 1 byte
    pub const BATTLE_TYPE: u16 = 0xD230;        // 1 byte
    pub const BATTLE_RESULT: u16 = 0xD0EE;      // 1 byte

    // Player's active Pokemon in battle
    pub const BATTLE_YOUR_SPECIES: u16 = 0xC62C;  // 1 byte
    pub const BATTLE_YOUR_MOVES: u16 = 0xC62E;    // 4 bytes
    pub const BATTLE_YOUR_PP: u16 = 0xC634;       // 4 bytes
    pub const BATTLE_YOUR_LEVEL: u16 = 0xC639;    // 1 byte
    pub const BATTLE_YOUR_HP: u16 = 0xC63C;       // 2 bytes
    pub const BATTLE_YOUR_HP_MAX: u16 = 0xC63E;   // 2 bytes
    pub const BATTLE_YOUR_ATTACK: u16 = 0xC640;   // 2 bytes
    pub const BATTLE_YOUR_DEFENSE: u16 = 0xC642;  // 2 bytes
    pub const BATTLE_YOUR_SPEED: u16 = 0xC644;    // 2 bytes
    pub const BATTLE_YOUR_SP_ATK: u16 = 0xC646;   // 2 bytes
    pub const BATTLE_YOUR_SP_DEF: u16 = 0xC648;   // 2 bytes

    // Enemy Pokemon in battle
    pub const ENEMY_MON_SPECIES: u16 = 0xD206;    // 1 byte
    pub const ENEMY_MON_MOVES: u16 = 0xD208;      // 4 bytes
    pub const ENEMY_MON_PP: u16 = 0xD20E;         // 4 bytes
    pub const ENEMY_MON_LEVEL: u16 = 0xD213;      // 1 byte
    pub const ENEMY_MON_HP: u16 = 0xD218;         // 2 bytes
    pub const ENEMY_MON_HP_MAX: u16 = 0xD21A;     // 2 bytes

    // --- Text Encoding (Gen 2) ---
    pub const CHAR_TERMINATOR: u8 = 0x50;
    pub const CHAR_SPACE: u8 = 0x7F;
}

// =============================================================================
// Status Condition Flags
// =============================================================================

/// Pokemon status conditions (same for Gen 1 and Gen 2)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StatusCondition {
    None = 0,
    Sleep1 = 1,
    Sleep2 = 2,
    Sleep3 = 3,
    Sleep4 = 4,
    Sleep5 = 5,
    Sleep6 = 6,
    Sleep7 = 7,
    Poison = 0x08,
    Burn = 0x10,
    Freeze = 0x20,
    Paralysis = 0x40,
}

impl StatusCondition {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0 => Self::None,
            1..=7 => unsafe { core::mem::transmute(b) }, // Sleep turns
            0x08 => Self::Poison,
            0x10 => Self::Burn,
            0x20 => Self::Freeze,
            0x40 => Self::Paralysis,
            _ => Self::None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "",
            Self::Sleep1 | Self::Sleep2 | Self::Sleep3 |
            Self::Sleep4 | Self::Sleep5 | Self::Sleep6 | Self::Sleep7 => "SLP",
            Self::Poison => "PSN",
            Self::Burn => "BRN",
            Self::Freeze => "FRZ",
            Self::Paralysis => "PAR",
        }
    }

    pub fn is_sleeping(&self) -> bool {
        matches!(self, Self::Sleep1 | Self::Sleep2 | Self::Sleep3 |
                       Self::Sleep4 | Self::Sleep5 | Self::Sleep6 | Self::Sleep7)
    }
}

// =============================================================================
// Pokemon Types
// =============================================================================

/// Pokemon types (Gen 1/2 encoding)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PokemonType {
    Normal = 0x00,
    Fighting = 0x01,
    Flying = 0x02,
    Poison = 0x03,
    Ground = 0x04,
    Rock = 0x05,
    Bird = 0x06,      // Unused/glitch
    Bug = 0x07,
    Ghost = 0x08,
    Steel = 0x09,     // Gen 2 only
    // 0x0A-0x13 unused
    Fire = 0x14,
    Water = 0x15,
    Grass = 0x16,
    Electric = 0x17,
    Psychic = 0x18,
    Ice = 0x19,
    Dragon = 0x1A,
    Dark = 0x1B,      // Gen 2 only
    Unknown = 0xFF,
}

impl PokemonType {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0x00 => Self::Normal,
            0x01 => Self::Fighting,
            0x02 => Self::Flying,
            0x03 => Self::Poison,
            0x04 => Self::Ground,
            0x05 => Self::Rock,
            0x06 => Self::Bird,
            0x07 => Self::Bug,
            0x08 => Self::Ghost,
            0x09 => Self::Steel,
            0x14 => Self::Fire,
            0x15 => Self::Water,
            0x16 => Self::Grass,
            0x17 => Self::Electric,
            0x18 => Self::Psychic,
            0x19 => Self::Ice,
            0x1A => Self::Dragon,
            0x1B => Self::Dark,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Fighting => "FIGHTING",
            Self::Flying => "FLYING",
            Self::Poison => "POISON",
            Self::Ground => "GROUND",
            Self::Rock => "ROCK",
            Self::Bird => "BIRD",
            Self::Bug => "BUG",
            Self::Ghost => "GHOST",
            Self::Steel => "STEEL",
            Self::Fire => "FIRE",
            Self::Water => "WATER",
            Self::Grass => "GRASS",
            Self::Electric => "ELECTRIC",
            Self::Psychic => "PSYCHIC",
            Self::Ice => "ICE",
            Self::Dragon => "DRAGON",
            Self::Dark => "DARK",
            Self::Unknown => "???",
        }
    }
}

// =============================================================================
// High-Level Data Structures
// =============================================================================

/// Pokemon data extracted from RAM
#[derive(Debug, Clone)]
pub struct Pokemon {
    pub species: u8,
    pub level: u8,
    pub hp_current: u16,
    pub hp_max: u16,
    pub status: StatusCondition,
    pub attack: u16,
    pub defense: u16,
    pub speed: u16,
    pub special: u16,       // Gen 1, or Sp.Atk for Gen 2
    pub special_def: u16,   // Gen 2 only (0 for Gen 1)
    pub moves: [u8; 4],
    pub pp: [u8; 4],
    pub held_item: u8,      // Gen 2 only (0 for Gen 1)
    pub friendship: u8,     // Gen 2 only (0 for Gen 1)
}

impl Pokemon {
    pub fn is_fainted(&self) -> bool {
        self.hp_current == 0
    }

    pub fn hp_percent(&self) -> u8 {
        if self.hp_max == 0 { return 0; }
        ((self.hp_current as u32 * 100) / self.hp_max as u32) as u8
    }
}

/// Battle Pokemon data - active Pokemon during battle
/// Contains in-battle stats (with badge boosts/modifiers applied)
#[derive(Debug, Clone)]
pub struct BattlePokemon {
    pub species: u8,
    pub level: u8,
    pub hp_current: u16,
    pub hp_max: u16,
    pub attack: u16,
    pub defense: u16,
    pub speed: u16,
    pub special: u16,        // Gen 1, or Sp.Atk for Gen 2
    pub special_def: u16,    // Gen 2 only (0 for Gen 1)
    pub moves: [u8; 4],
    pub pp: [u8; 4],
}

impl BattlePokemon {
    pub fn is_valid(&self) -> bool {
        self.species != 0 && self.species != 0xFF
    }

    pub fn hp_percent(&self) -> u8 {
        if self.hp_max == 0 { return 0; }
        ((self.hp_current as u32 * 100) / self.hp_max as u32) as u8
    }
}

/// Trainer/player data extracted from RAM
#[derive(Debug, Clone)]
pub struct TrainerData {
    pub name: [u8; 11],
    pub money: u32,
    pub badges: u8,
    pub badges_kanto: u8,   // Gen 2 only
    pub play_hours: u16,
    pub play_minutes: u8,
    pub play_seconds: u8,
    pub pokedex_owned: u8,  // Count of owned Pokemon
    pub pokedex_seen: u8,   // Count of seen Pokemon
}

/// Complete party state
#[derive(Debug, Clone)]
pub struct PartyState {
    pub count: u8,
    pub pokemon: [Option<Pokemon>; 6],
}

// =============================================================================
// RAM Reader Implementation
// =============================================================================

/// Reads game state from MMU using peek (no side effects)
pub struct RamReader<'a> {
    mmu: &'a MMU,
    game: Game,
}

impl<'a> RamReader<'a> {
    pub fn new(mmu: &'a MMU, game: Game) -> Self {
        Self { mmu, game }
    }

    /// Get the detected game
    pub fn game(&self) -> Game {
        self.game
    }

    /// Read a single byte
    fn rb(&self, addr: u16) -> u8 {
        self.mmu.peek(addr)
    }

    /// Read a 16-bit word (little-endian)
    fn rw(&self, addr: u16) -> u16 {
        self.mmu.peek_word(addr)
    }

    /// Read 3-byte BCD value as integer (for Gen 1 money)
    fn read_bcd24(&self, addr: u16) -> u32 {
        let b0 = self.rb(addr) as u32;
        let b1 = self.rb(addr + 1) as u32;
        let b2 = self.rb(addr + 2) as u32;

        // BCD: each nibble is a digit
        let d5 = (b0 >> 4) & 0xF;
        let d4 = b0 & 0xF;
        let d3 = (b1 >> 4) & 0xF;
        let d2 = b1 & 0xF;
        let d1 = (b2 >> 4) & 0xF;
        let d0 = b2 & 0xF;

        d5 * 100000 + d4 * 10000 + d3 * 1000 + d2 * 100 + d1 * 10 + d0
    }

    /// Read 3-byte big-endian integer (for Gen 2 money)
    fn read_int24(&self, addr: u16) -> u32 {
        let b0 = self.rb(addr) as u32;
        let b1 = self.rb(addr + 1) as u32;
        let b2 = self.rb(addr + 2) as u32;
        (b0 << 16) | (b1 << 8) | b2
    }

    /// Count set bits in a byte range (for Pokedex)
    fn count_bits(&self, addr: u16, len: u16) -> u8 {
        let mut count = 0u8;
        for i in 0..len {
            let byte = self.rb(addr + i);
            count = count.saturating_add(byte.count_ones() as u8);
        }
        count
    }

    /// Read party count
    pub fn party_count(&self) -> u8 {
        let addr = match self.game {
            Game::Red | Game::Blue => red_blue::PARTY_COUNT,
            Game::Yellow => yellow::PARTY_COUNT,
            Game::Gold | Game::Silver => gold_silver::PARTY_COUNT,
            Game::Crystal => crystal::PARTY_COUNT,
            Game::Unknown => return 0,
        };
        self.rb(addr).min(6)
    }

    /// Read a party Pokemon's data
    pub fn read_party_pokemon(&self, slot: u8) -> Option<Pokemon> {
        if slot >= 6 { return None; }

        let count = self.party_count();
        if slot >= count { return None; }

        match self.game {
            Game::Red | Game::Blue => self.read_pokemon_gen1_rb(slot),
            Game::Yellow => self.read_pokemon_gen1_yellow(slot),
            Game::Gold | Game::Silver => self.read_pokemon_gen2_gs(slot),
            Game::Crystal => self.read_pokemon_gen2_crystal(slot),
            Game::Unknown => None,
        }
    }

    fn read_pokemon_gen1_rb(&self, slot: u8) -> Option<Pokemon> {
        let base = red_blue::PARTY_DATA + (slot as u16 * red_blue::PARTY_MON_SIZE);

        let species = self.rb(base + red_blue::MON_SPECIES);
        if species == 0 || species == 0xFF { return None; }

        Some(Pokemon {
            species,
            level: self.rb(base + red_blue::MON_LEVEL),
            hp_current: self.rw(base + red_blue::MON_HP_CURRENT),
            hp_max: self.rw(base + red_blue::MON_HP_MAX),
            status: StatusCondition::from_byte(self.rb(base + red_blue::MON_STATUS)),
            attack: self.rw(base + red_blue::MON_ATTACK),
            defense: self.rw(base + red_blue::MON_DEFENSE),
            speed: self.rw(base + red_blue::MON_SPEED),
            special: self.rw(base + red_blue::MON_SPECIAL),
            special_def: 0, // Gen 1 has unified Special
            moves: [
                self.rb(base + red_blue::MON_MOVES),
                self.rb(base + red_blue::MON_MOVES + 1),
                self.rb(base + red_blue::MON_MOVES + 2),
                self.rb(base + red_blue::MON_MOVES + 3),
            ],
            pp: [
                self.rb(base + red_blue::MON_PP) & 0x3F,
                self.rb(base + red_blue::MON_PP + 1) & 0x3F,
                self.rb(base + red_blue::MON_PP + 2) & 0x3F,
                self.rb(base + red_blue::MON_PP + 3) & 0x3F,
            ],
            held_item: 0, // Gen 1 doesn't have held items
            friendship: 0,
        })
    }

    fn read_pokemon_gen1_yellow(&self, slot: u8) -> Option<Pokemon> {
        let base = yellow::PARTY_DATA + (slot as u16 * yellow::PARTY_MON_SIZE);

        let species = self.rb(base + yellow::MON_SPECIES);
        if species == 0 || species == 0xFF { return None; }

        Some(Pokemon {
            species,
            level: self.rb(base + yellow::MON_LEVEL),
            hp_current: self.rw(base + yellow::MON_HP_CURRENT),
            hp_max: self.rw(base + yellow::MON_HP_MAX),
            status: StatusCondition::from_byte(self.rb(base + yellow::MON_STATUS)),
            attack: self.rw(base + yellow::MON_ATTACK),
            defense: self.rw(base + yellow::MON_DEFENSE),
            speed: self.rw(base + yellow::MON_SPEED),
            special: self.rw(base + yellow::MON_SPECIAL),
            special_def: 0,
            moves: [
                self.rb(base + yellow::MON_MOVES),
                self.rb(base + yellow::MON_MOVES + 1),
                self.rb(base + yellow::MON_MOVES + 2),
                self.rb(base + yellow::MON_MOVES + 3),
            ],
            pp: [
                self.rb(base + yellow::MON_PP) & 0x3F,
                self.rb(base + yellow::MON_PP + 1) & 0x3F,
                self.rb(base + yellow::MON_PP + 2) & 0x3F,
                self.rb(base + yellow::MON_PP + 3) & 0x3F,
            ],
            held_item: 0,
            friendship: 0,
        })
    }

    fn read_pokemon_gen2_gs(&self, slot: u8) -> Option<Pokemon> {
        let base = gold_silver::PARTY_DATA + (slot as u16 * gold_silver::PARTY_MON_SIZE);

        let species = self.rb(base + gold_silver::MON_SPECIES);
        if species == 0 || species == 0xFF { return None; }

        Some(Pokemon {
            species,
            level: self.rb(base + gold_silver::MON_LEVEL),
            hp_current: self.rw(base + gold_silver::MON_HP_CURRENT),
            hp_max: self.rw(base + gold_silver::MON_HP_MAX),
            status: StatusCondition::from_byte(self.rb(base + gold_silver::MON_STATUS)),
            attack: self.rw(base + gold_silver::MON_ATTACK),
            defense: self.rw(base + gold_silver::MON_DEFENSE),
            speed: self.rw(base + gold_silver::MON_SPEED),
            special: self.rw(base + gold_silver::MON_SP_ATK),
            special_def: self.rw(base + gold_silver::MON_SP_DEF),
            moves: [
                self.rb(base + gold_silver::MON_MOVES),
                self.rb(base + gold_silver::MON_MOVES + 1),
                self.rb(base + gold_silver::MON_MOVES + 2),
                self.rb(base + gold_silver::MON_MOVES + 3),
            ],
            pp: [
                self.rb(base + gold_silver::MON_PP) & 0x3F,
                self.rb(base + gold_silver::MON_PP + 1) & 0x3F,
                self.rb(base + gold_silver::MON_PP + 2) & 0x3F,
                self.rb(base + gold_silver::MON_PP + 3) & 0x3F,
            ],
            held_item: self.rb(base + gold_silver::MON_ITEM),
            friendship: self.rb(base + gold_silver::MON_FRIENDSHIP),
        })
    }

    fn read_pokemon_gen2_crystal(&self, slot: u8) -> Option<Pokemon> {
        let base = crystal::PARTY_DATA + (slot as u16 * crystal::PARTY_MON_SIZE);

        let species = self.rb(base + crystal::MON_SPECIES);
        if species == 0 || species == 0xFF { return None; }

        Some(Pokemon {
            species,
            level: self.rb(base + crystal::MON_LEVEL),
            hp_current: self.rw(base + crystal::MON_HP_CURRENT),
            hp_max: self.rw(base + crystal::MON_HP_MAX),
            status: StatusCondition::from_byte(self.rb(base + crystal::MON_STATUS)),
            attack: self.rw(base + crystal::MON_ATTACK),
            defense: self.rw(base + crystal::MON_DEFENSE),
            speed: self.rw(base + crystal::MON_SPEED),
            special: self.rw(base + crystal::MON_SP_ATK),
            special_def: self.rw(base + crystal::MON_SP_DEF),
            moves: [
                self.rb(base + crystal::MON_MOVES),
                self.rb(base + crystal::MON_MOVES + 1),
                self.rb(base + crystal::MON_MOVES + 2),
                self.rb(base + crystal::MON_MOVES + 3),
            ],
            pp: [
                self.rb(base + crystal::MON_PP) & 0x3F,
                self.rb(base + crystal::MON_PP + 1) & 0x3F,
                self.rb(base + crystal::MON_PP + 2) & 0x3F,
                self.rb(base + crystal::MON_PP + 3) & 0x3F,
            ],
            held_item: self.rb(base + crystal::MON_ITEM),
            friendship: self.rb(base + crystal::MON_FRIENDSHIP),
        })
    }

    /// Read full party state
    pub fn read_party(&self) -> PartyState {
        let count = self.party_count();
        let mut pokemon = [None, None, None, None, None, None];

        for i in 0..count.min(6) {
            pokemon[i as usize] = self.read_party_pokemon(i);
        }

        PartyState { count, pokemon }
    }

    /// Read trainer/player data
    pub fn read_trainer(&self) -> TrainerData {
        match self.game {
            Game::Red | Game::Blue => self.read_trainer_gen1_rb(),
            Game::Yellow => self.read_trainer_gen1_yellow(),
            Game::Gold | Game::Silver => self.read_trainer_gen2_gs(),
            Game::Crystal => self.read_trainer_gen2_crystal(),
            Game::Unknown => TrainerData {
                name: [0; 11],
                money: 0,
                badges: 0,
                badges_kanto: 0,
                play_hours: 0,
                play_minutes: 0,
                play_seconds: 0,
                pokedex_owned: 0,
                pokedex_seen: 0,
            },
        }
    }

    fn read_trainer_gen1_rb(&self) -> TrainerData {
        let mut name = [0u8; 11];
        for i in 0..11 {
            name[i] = self.rb(red_blue::PLAYER_NAME + i as u16);
        }

        TrainerData {
            name,
            money: self.read_bcd24(red_blue::MONEY),
            badges: self.rb(red_blue::BADGES),
            badges_kanto: 0,
            play_hours: self.rw(red_blue::PLAY_TIME_HOURS),
            play_minutes: self.rb(red_blue::PLAY_TIME_MINUTES),
            play_seconds: self.rb(red_blue::PLAY_TIME_SECONDS),
            pokedex_owned: self.count_bits(red_blue::POKEDEX_OWNED, 19),
            pokedex_seen: self.count_bits(red_blue::POKEDEX_SEEN, 19),
        }
    }

    fn read_trainer_gen1_yellow(&self) -> TrainerData {
        let mut name = [0u8; 11];
        for i in 0..11 {
            name[i] = self.rb(yellow::PLAYER_NAME + i as u16);
        }

        TrainerData {
            name,
            money: self.read_bcd24(yellow::MONEY),
            badges: self.rb(yellow::BADGES),
            badges_kanto: 0,
            play_hours: self.rw(yellow::PLAY_TIME_HOURS),
            play_minutes: self.rb(yellow::PLAY_TIME_MINUTES),
            play_seconds: self.rb(yellow::PLAY_TIME_SECONDS),
            pokedex_owned: self.count_bits(yellow::POKEDEX_OWNED, 19),
            pokedex_seen: self.count_bits(yellow::POKEDEX_SEEN, 19),
        }
    }

    fn read_trainer_gen2_gs(&self) -> TrainerData {
        let mut name = [0u8; 11];
        for i in 0..11 {
            name[i] = self.rb(gold_silver::PLAYER_NAME + i as u16);
        }

        TrainerData {
            name,
            money: self.read_int24(gold_silver::MONEY),  // Gen 2 uses regular int, not BCD
            badges: self.rb(gold_silver::JOHTO_BADGES),
            badges_kanto: self.rb(gold_silver::KANTO_BADGES),
            play_hours: self.rw(gold_silver::PLAY_TIME_HOURS),
            play_minutes: self.rb(gold_silver::PLAY_TIME_MINUTES),
            play_seconds: self.rb(gold_silver::PLAY_TIME_SECONDS),
            pokedex_owned: self.count_bits(gold_silver::POKEDEX_OWNED, 32),
            pokedex_seen: self.count_bits(gold_silver::POKEDEX_SEEN, 32),
        }
    }

    fn read_trainer_gen2_crystal(&self) -> TrainerData {
        let mut name = [0u8; 11];
        for i in 0..11 {
            name[i] = self.rb(crystal::PLAYER_NAME + i as u16);
        }

        TrainerData {
            name,
            money: self.read_int24(crystal::MONEY),  // Gen 2 uses regular int, not BCD
            badges: self.rb(crystal::JOHTO_BADGES),
            badges_kanto: self.rb(crystal::KANTO_BADGES),
            play_hours: self.rw(crystal::PLAY_TIME_HOURS),
            play_minutes: self.rb(crystal::PLAY_TIME_MINUTES),
            play_seconds: self.rb(crystal::PLAY_TIME_SECONDS),
            pokedex_owned: self.count_bits(crystal::POKEDEX_OWNED, 32),
            pokedex_seen: self.count_bits(crystal::POKEDEX_SEEN, 32),
        }
    }

    /// Check if currently in battle
    pub fn in_battle(&self) -> bool {
        match self.game {
            Game::Red | Game::Blue => self.rb(red_blue::IN_BATTLE) != 0,
            Game::Yellow => self.rb(yellow::IN_BATTLE) != 0,
            Game::Gold | Game::Silver => self.rb(gold_silver::BATTLE_MODE) != 0,
            Game::Crystal => self.rb(crystal::BATTLE_MODE) != 0,
            Game::Unknown => false,
        }
    }

    /// Get enemy Pokemon info during battle (species, HP, level)
    pub fn read_enemy_pokemon(&self) -> Option<(u8, u16, u8)> {
        if !self.in_battle() { return None; }

        let (species_addr, hp_addr, level_addr) = match self.game {
            Game::Red | Game::Blue => (
                red_blue::ENEMY_MON_SPECIES,
                red_blue::ENEMY_MON_HP,
                red_blue::ENEMY_MON_LEVEL
            ),
            Game::Yellow => (
                yellow::ENEMY_MON_SPECIES,
                yellow::ENEMY_MON_HP,
                yellow::ENEMY_MON_LEVEL
            ),
            Game::Gold | Game::Silver => (
                gold_silver::ENEMY_MON_SPECIES,
                gold_silver::ENEMY_MON_HP,
                gold_silver::ENEMY_MON_LEVEL
            ),
            Game::Crystal => (
                crystal::ENEMY_MON_SPECIES,
                crystal::ENEMY_MON_HP,
                crystal::ENEMY_MON_LEVEL
            ),
            Game::Unknown => return None,
        };

        let species = self.rb(species_addr);
        if species == 0 { return None; }

        Some((
            species,
            self.rw(hp_addr),
            self.rb(level_addr),
        ))
    }

    /// Read player location (map, x, y)
    pub fn read_location(&self) -> (u8, u8, u8) {
        match self.game {
            Game::Red | Game::Blue => (
                self.rb(red_blue::CURRENT_MAP),
                self.rb(red_blue::MAP_X),
                self.rb(red_blue::MAP_Y),
            ),
            Game::Yellow => (
                self.rb(yellow::CURRENT_MAP),
                self.rb(yellow::MAP_X),
                self.rb(yellow::MAP_Y),
            ),
            Game::Gold | Game::Silver => (
                self.rb(gold_silver::MAP_NUMBER),
                self.rb(gold_silver::MAP_X),
                self.rb(gold_silver::MAP_Y),
            ),
            Game::Crystal => (
                self.rb(crystal::MAP_NUMBER),
                self.rb(crystal::MAP_X),
                self.rb(crystal::MAP_Y),
            ),
            Game::Unknown => (0, 0, 0),
        }
    }

    /// Read Pikachu happiness (Yellow only)
    pub fn read_pikachu_happiness(&self) -> Option<u8> {
        if self.game == Game::Yellow {
            Some(self.rb(yellow::PIKACHU_HAPPINESS))
        } else {
            None
        }
    }

    /// Read player gender (Crystal only, returns None for other games)
    pub fn read_player_gender(&self) -> Option<u8> {
        if self.game == Game::Crystal {
            Some(self.rb(crystal::PLAYER_GENDER))
        } else {
            None
        }
    }

    /// Read active (your) Pokemon in battle
    pub fn read_battle_your_pokemon(&self) -> Option<BattlePokemon> {
        if !self.in_battle() { return None; }

        match self.game {
            Game::Red | Game::Blue => self.read_battle_your_gen1_rb(),
            Game::Yellow => self.read_battle_your_gen1_yellow(),
            Game::Gold | Game::Silver => self.read_battle_your_gen2_gs(),
            Game::Crystal => self.read_battle_your_gen2_crystal(),
            Game::Unknown => None,
        }
    }

    fn read_battle_your_gen1_rb(&self) -> Option<BattlePokemon> {
        let species = self.rb(red_blue::BATTLE_YOUR_SPECIES);
        if species == 0 || species == 0xFF { return None; }

        Some(BattlePokemon {
            species,
            level: self.rb(red_blue::BATTLE_YOUR_LEVEL),
            hp_current: self.rw(red_blue::BATTLE_YOUR_HP),
            hp_max: self.rw(red_blue::BATTLE_YOUR_HP_MAX),
            attack: self.rw(red_blue::BATTLE_YOUR_ATTACK),
            defense: self.rw(red_blue::BATTLE_YOUR_DEFENSE),
            speed: self.rw(red_blue::BATTLE_YOUR_SPEED),
            special: self.rw(red_blue::BATTLE_YOUR_SPECIAL),
            special_def: 0,
            moves: [
                self.rb(red_blue::BATTLE_YOUR_MOVES),
                self.rb(red_blue::BATTLE_YOUR_MOVES + 1),
                self.rb(red_blue::BATTLE_YOUR_MOVES + 2),
                self.rb(red_blue::BATTLE_YOUR_MOVES + 3),
            ],
            pp: [
                self.rb(red_blue::BATTLE_YOUR_PP) & 0x3F,
                self.rb(red_blue::BATTLE_YOUR_PP + 1) & 0x3F,
                self.rb(red_blue::BATTLE_YOUR_PP + 2) & 0x3F,
                self.rb(red_blue::BATTLE_YOUR_PP + 3) & 0x3F,
            ],
        })
    }

    fn read_battle_your_gen1_yellow(&self) -> Option<BattlePokemon> {
        let species = self.rb(yellow::BATTLE_YOUR_SPECIES);
        if species == 0 || species == 0xFF { return None; }

        Some(BattlePokemon {
            species,
            level: self.rb(yellow::BATTLE_YOUR_LEVEL),
            hp_current: self.rw(yellow::BATTLE_YOUR_HP),
            hp_max: self.rw(yellow::BATTLE_YOUR_HP_MAX),
            attack: self.rw(yellow::BATTLE_YOUR_ATTACK),
            defense: self.rw(yellow::BATTLE_YOUR_DEFENSE),
            speed: self.rw(yellow::BATTLE_YOUR_SPEED),
            special: self.rw(yellow::BATTLE_YOUR_SPECIAL),
            special_def: 0,
            moves: [
                self.rb(yellow::BATTLE_YOUR_MOVES),
                self.rb(yellow::BATTLE_YOUR_MOVES + 1),
                self.rb(yellow::BATTLE_YOUR_MOVES + 2),
                self.rb(yellow::BATTLE_YOUR_MOVES + 3),
            ],
            pp: [
                self.rb(yellow::BATTLE_YOUR_PP) & 0x3F,
                self.rb(yellow::BATTLE_YOUR_PP + 1) & 0x3F,
                self.rb(yellow::BATTLE_YOUR_PP + 2) & 0x3F,
                self.rb(yellow::BATTLE_YOUR_PP + 3) & 0x3F,
            ],
        })
    }

    fn read_battle_your_gen2_gs(&self) -> Option<BattlePokemon> {
        let species = self.rb(gold_silver::BATTLE_YOUR_SPECIES);
        if species == 0 || species == 0xFF { return None; }

        Some(BattlePokemon {
            species,
            level: self.rb(gold_silver::BATTLE_YOUR_LEVEL),
            hp_current: self.rw(gold_silver::BATTLE_YOUR_HP),
            hp_max: self.rw(gold_silver::BATTLE_YOUR_HP_MAX),
            attack: self.rw(gold_silver::BATTLE_YOUR_ATTACK),
            defense: self.rw(gold_silver::BATTLE_YOUR_DEFENSE),
            speed: self.rw(gold_silver::BATTLE_YOUR_SPEED),
            special: self.rw(gold_silver::BATTLE_YOUR_SP_ATK),
            special_def: self.rw(gold_silver::BATTLE_YOUR_SP_DEF),
            moves: [
                self.rb(gold_silver::BATTLE_YOUR_MOVES),
                self.rb(gold_silver::BATTLE_YOUR_MOVES + 1),
                self.rb(gold_silver::BATTLE_YOUR_MOVES + 2),
                self.rb(gold_silver::BATTLE_YOUR_MOVES + 3),
            ],
            pp: [
                self.rb(gold_silver::BATTLE_YOUR_PP) & 0x3F,
                self.rb(gold_silver::BATTLE_YOUR_PP + 1) & 0x3F,
                self.rb(gold_silver::BATTLE_YOUR_PP + 2) & 0x3F,
                self.rb(gold_silver::BATTLE_YOUR_PP + 3) & 0x3F,
            ],
        })
    }

    fn read_battle_your_gen2_crystal(&self) -> Option<BattlePokemon> {
        let species = self.rb(crystal::BATTLE_YOUR_SPECIES);
        if species == 0 || species == 0xFF { return None; }

        Some(BattlePokemon {
            species,
            level: self.rb(crystal::BATTLE_YOUR_LEVEL),
            hp_current: self.rw(crystal::BATTLE_YOUR_HP),
            hp_max: self.rw(crystal::BATTLE_YOUR_HP_MAX),
            attack: self.rw(crystal::BATTLE_YOUR_ATTACK),
            defense: self.rw(crystal::BATTLE_YOUR_DEFENSE),
            speed: self.rw(crystal::BATTLE_YOUR_SPEED),
            special: self.rw(crystal::BATTLE_YOUR_SP_ATK),
            special_def: self.rw(crystal::BATTLE_YOUR_SP_DEF),
            moves: [
                self.rb(crystal::BATTLE_YOUR_MOVES),
                self.rb(crystal::BATTLE_YOUR_MOVES + 1),
                self.rb(crystal::BATTLE_YOUR_MOVES + 2),
                self.rb(crystal::BATTLE_YOUR_MOVES + 3),
            ],
            pp: [
                self.rb(crystal::BATTLE_YOUR_PP) & 0x3F,
                self.rb(crystal::BATTLE_YOUR_PP + 1) & 0x3F,
                self.rb(crystal::BATTLE_YOUR_PP + 2) & 0x3F,
                self.rb(crystal::BATTLE_YOUR_PP + 3) & 0x3F,
            ],
        })
    }

    /// Read enemy Pokemon in battle (full data)
    pub fn read_battle_enemy_pokemon(&self) -> Option<BattlePokemon> {
        if !self.in_battle() { return None; }

        match self.game {
            Game::Red | Game::Blue => self.read_battle_enemy_gen1_rb(),
            Game::Yellow => self.read_battle_enemy_gen1_yellow(),
            Game::Gold | Game::Silver => self.read_battle_enemy_gen2_gs(),
            Game::Crystal => self.read_battle_enemy_gen2_crystal(),
            Game::Unknown => None,
        }
    }

    fn read_battle_enemy_gen1_rb(&self) -> Option<BattlePokemon> {
        let species = self.rb(red_blue::ENEMY_MON_SPECIES);
        if species == 0 || species == 0xFF { return None; }

        Some(BattlePokemon {
            species,
            level: self.rb(red_blue::ENEMY_MON_LEVEL),
            hp_current: self.rw(red_blue::ENEMY_MON_HP),
            hp_max: self.rw(red_blue::ENEMY_MON_HP_MAX),
            attack: self.rw(red_blue::ENEMY_MON_ATTACK),
            defense: self.rw(red_blue::ENEMY_MON_DEFENSE),
            speed: self.rw(red_blue::ENEMY_MON_SPEED),
            special: self.rw(red_blue::ENEMY_MON_SPECIAL),
            special_def: 0,
            moves: [
                self.rb(red_blue::ENEMY_MON_MOVES),
                self.rb(red_blue::ENEMY_MON_MOVES + 1),
                self.rb(red_blue::ENEMY_MON_MOVES + 2),
                self.rb(red_blue::ENEMY_MON_MOVES + 3),
            ],
            pp: [0, 0, 0, 0], // Enemy PP not tracked in Gen 1
        })
    }

    fn read_battle_enemy_gen1_yellow(&self) -> Option<BattlePokemon> {
        let species = self.rb(yellow::ENEMY_MON_SPECIES);
        if species == 0 || species == 0xFF { return None; }

        Some(BattlePokemon {
            species,
            level: self.rb(yellow::ENEMY_MON_LEVEL),
            hp_current: self.rw(yellow::ENEMY_MON_HP),
            hp_max: self.rw(yellow::ENEMY_MON_HP_MAX),
            attack: self.rw(yellow::ENEMY_MON_ATTACK),
            defense: self.rw(yellow::ENEMY_MON_DEFENSE),
            speed: self.rw(yellow::ENEMY_MON_SPEED),
            special: self.rw(yellow::ENEMY_MON_SPECIAL),
            special_def: 0,
            moves: [
                self.rb(yellow::ENEMY_MON_MOVES),
                self.rb(yellow::ENEMY_MON_MOVES + 1),
                self.rb(yellow::ENEMY_MON_MOVES + 2),
                self.rb(yellow::ENEMY_MON_MOVES + 3),
            ],
            pp: [0, 0, 0, 0], // Enemy PP not tracked in Gen 1
        })
    }

    fn read_battle_enemy_gen2_gs(&self) -> Option<BattlePokemon> {
        let species = self.rb(gold_silver::ENEMY_MON_SPECIES);
        if species == 0 || species == 0xFF { return None; }

        Some(BattlePokemon {
            species,
            level: self.rb(gold_silver::ENEMY_MON_LEVEL),
            hp_current: self.rw(gold_silver::ENEMY_MON_HP),
            hp_max: self.rw(gold_silver::ENEMY_MON_HP_MAX),
            attack: self.rw(gold_silver::ENEMY_MON_ATTACK),
            defense: self.rw(gold_silver::ENEMY_MON_DEFENSE),
            speed: self.rw(gold_silver::ENEMY_MON_SPEED),
            special: self.rw(gold_silver::ENEMY_MON_SP_ATK),
            special_def: self.rw(gold_silver::ENEMY_MON_SP_DEF),
            moves: [
                self.rb(gold_silver::ENEMY_MON_MOVES),
                self.rb(gold_silver::ENEMY_MON_MOVES + 1),
                self.rb(gold_silver::ENEMY_MON_MOVES + 2),
                self.rb(gold_silver::ENEMY_MON_MOVES + 3),
            ],
            pp: [
                self.rb(gold_silver::ENEMY_MON_PP) & 0x3F,
                self.rb(gold_silver::ENEMY_MON_PP + 1) & 0x3F,
                self.rb(gold_silver::ENEMY_MON_PP + 2) & 0x3F,
                self.rb(gold_silver::ENEMY_MON_PP + 3) & 0x3F,
            ],
        })
    }

    fn read_battle_enemy_gen2_crystal(&self) -> Option<BattlePokemon> {
        let species = self.rb(crystal::ENEMY_MON_SPECIES);
        if species == 0 || species == 0xFF { return None; }

        Some(BattlePokemon {
            species,
            level: self.rb(crystal::ENEMY_MON_LEVEL),
            hp_current: self.rw(crystal::ENEMY_MON_HP),
            hp_max: self.rw(crystal::ENEMY_MON_HP_MAX),
            attack: 0,   // Crystal doesn't expose enemy stats in accessible location
            defense: 0,
            speed: 0,
            special: 0,
            special_def: 0,
            moves: [
                self.rb(crystal::ENEMY_MON_MOVES),
                self.rb(crystal::ENEMY_MON_MOVES + 1),
                self.rb(crystal::ENEMY_MON_MOVES + 2),
                self.rb(crystal::ENEMY_MON_MOVES + 3),
            ],
            pp: [
                self.rb(crystal::ENEMY_MON_PP) & 0x3F,
                self.rb(crystal::ENEMY_MON_PP + 1) & 0x3F,
                self.rb(crystal::ENEMY_MON_PP + 2) & 0x3F,
                self.rb(crystal::ENEMY_MON_PP + 3) & 0x3F,
            ],
        })
    }

    /// Read map group for Gen 2 games
    /// Returns 0 for Gen 1 games
    pub fn read_map_group(&self) -> u8 {
        match self.game {
            Game::Gold | Game::Silver => self.rb(gold_silver::MAP_GROUP),
            Game::Crystal => self.rb(crystal::MAP_GROUP),
            _ => 0,
        }
    }

    /// Read battle type
    /// Returns: 0 = no battle, 1 = wild, 2 = trainer
    pub fn read_battle_type(&self) -> u8 {
        match self.game {
            Game::Red | Game::Blue => self.rb(red_blue::IN_BATTLE),
            Game::Yellow => self.rb(yellow::IN_BATTLE),
            Game::Gold | Game::Silver => {
                if self.rb(gold_silver::BATTLE_MODE) != 0 {
                    self.rb(gold_silver::BATTLE_TYPE)
                } else {
                    0
                }
            }
            Game::Crystal => {
                if self.rb(crystal::BATTLE_MODE) != 0 {
                    self.rb(crystal::BATTLE_TYPE)
                } else {
                    0
                }
            }
            Game::Unknown => 0,
        }
    }

    /// Read enemy trainer's party count (for trainer battles)
    pub fn read_enemy_party_count(&self) -> u8 {
        if !self.in_battle() {
            return 0;
        }

        match self.game {
            Game::Red | Game::Blue => self.rb(red_blue::ENEMY_PARTY_COUNT),
            Game::Yellow => self.rb(yellow::ENEMY_PARTY_COUNT),
            Game::Gold | Game::Silver => self.rb(gold_silver::ENEMY_PARTY_COUNT),
            Game::Crystal => self.rb(crystal::ENEMY_PARTY_COUNT),
            Game::Unknown => 0,
        }
    }

    /// Read bag items (Gen 1)
    /// Returns: Vec of (item_id, quantity) pairs
    pub fn read_bag_gen1(&self) -> [(u8, u8); 20] {
        let mut items = [(0u8, 0u8); 20];

        let (count_addr, data_addr) = match self.game {
            Game::Red | Game::Blue => (red_blue::BAG_ITEM_COUNT, red_blue::BAG_ITEM_DATA),
            Game::Yellow => (yellow::BAG_ITEM_COUNT, yellow::BAG_ITEM_DATA),
            _ => return items,
        };

        let count = self.rb(count_addr).min(20);
        for i in 0..count as usize {
            let item_id = self.rb(data_addr + (i as u16 * 2));
            let quantity = self.rb(data_addr + (i as u16 * 2) + 1);
            items[i] = (item_id, quantity);
        }

        items
    }

    /// Read items pocket (Gen 2)
    /// Returns: Vec of (item_id, quantity) pairs
    pub fn read_items_pocket_gen2(&self) -> [(u8, u8); 20] {
        let mut items = [(0u8, 0u8); 20];

        if !self.game.is_gen2() {
            return items;
        }

        let (count_addr, data_addr) = match self.game {
            Game::Gold | Game::Silver => (gold_silver::ITEMS_POCKET_COUNT, gold_silver::ITEMS_POCKET_DATA),
            Game::Crystal => (crystal::ITEMS_POCKET_COUNT, crystal::ITEMS_POCKET_DATA),
            _ => return items,
        };

        let count = self.rb(count_addr).min(20);
        for i in 0..count as usize {
            let item_id = self.rb(data_addr + (i as u16 * 2));
            let quantity = self.rb(data_addr + (i as u16 * 2) + 1);
            items[i] = (item_id, quantity);
        }

        items
    }
}

// =============================================================================
// Text Decoding (Gen 1/2 character encoding)
// =============================================================================

/// Decode Pokemon text encoding to ASCII
pub fn decode_text(data: &[u8]) -> [u8; 11] {
    let mut result = [0u8; 11];

    for (i, &byte) in data.iter().enumerate() {
        if i >= 11 { break; }
        if byte == 0x50 { break; } // Terminator

        result[i] = match byte {
            0x7F => b' ',
            0x80..=0x99 => b'A' + (byte - 0x80), // A-Z
            0xA0..=0xB9 => b'a' + (byte - 0xA0), // a-z
            0xF6..=0xFF => b'0' + (byte - 0xF6), // 0-9
            0xE0 => b'\'',
            0xE3 => b'-',
            0xE8 => b'.',
            0xEF => b'!',
            0xF4 => b',',
            _ => b'?',
        };
    }

    result
}

/// Decode Pokemon text encoding to a static string buffer
pub fn decode_text_to_str(data: &[u8]) -> &'static str {
    // This is a simple implementation - in practice you'd want a proper
    // string buffer management system
    static mut BUFFER: [u8; 12] = [0; 12];

    unsafe {
        let decoded = decode_text(data);
        let mut len = 0;
        for (i, &b) in decoded.iter().enumerate() {
            if b == 0 { break; }
            BUFFER[i] = b;
            len = i + 1;
        }
        BUFFER[len] = 0;
        core::str::from_utf8_unchecked(&BUFFER[..len])
    }
}

// =============================================================================
// Helper Functions for Overlay System
// =============================================================================

/// Check if game is supported for overlay
pub fn is_game_supported(game: Game) -> bool {
    !matches!(game, Game::Unknown)
}

/// Get the Pokedex size for the game
pub fn pokedex_size(game: Game) -> u16 {
    match game {
        Game::Red | Game::Blue | Game::Yellow => 151,
        Game::Gold | Game::Silver | Game::Crystal => 251,
        Game::Unknown => 0,
    }
}

/// Get party Pokemon structure size
pub fn party_mon_size(game: Game) -> u16 {
    match game {
        Game::Red | Game::Blue | Game::Yellow => 44,
        Game::Gold | Game::Silver | Game::Crystal => 48,
        Game::Unknown => 0,
    }
}

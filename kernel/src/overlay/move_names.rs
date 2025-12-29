//! Pokemon Move Name Lookup Table
//!
//! Contains names for all 251 moves from Generation 1 and 2.
//! Move ID 0 = no move, 1-251 are valid moves.

/// Maximum move name length
pub const MAX_MOVE_NAME_LEN: usize = 12;

/// Get move name by move ID (1-251)
/// Returns "------" for invalid/empty move slots
pub fn get_move_name(move_id: u8) -> &'static str {
    if move_id == 0 || move_id as usize > MOVE_NAMES.len() {
        return "------";
    }
    MOVE_NAMES[move_id as usize - 1]
}

/// Get move name as bytes for direct rendering
pub fn get_move_name_bytes(move_id: u8) -> &'static [u8] {
    get_move_name(move_id).as_bytes()
}

/// All 251 move names (index 0 = move 1, etc.)
static MOVE_NAMES: [&str; 251] = [
    // Moves 001-025
    "POUND",
    "KARATE CHOP",
    "DOUBLESLAP",
    "COMET PUNCH",
    "MEGA PUNCH",
    "PAY DAY",
    "FIRE PUNCH",
    "ICE PUNCH",
    "THUNDERPUNCH",
    "SCRATCH",
    "VICEGRIP",
    "GUILLOTINE",
    "RAZOR WIND",
    "SWORDS DANCE",
    "CUT",
    "GUST",
    "WING ATTACK",
    "WHIRLWIND",
    "FLY",
    "BIND",
    "SLAM",
    "VINE WHIP",
    "STOMP",
    "DOUBLE KICK",
    "MEGA KICK",

    // Moves 026-050
    "JUMP KICK",
    "ROLLING KICK",
    "SAND-ATTACK",
    "HEADBUTT",
    "HORN ATTACK",
    "FURY ATTACK",
    "HORN DRILL",
    "TACKLE",
    "BODY SLAM",
    "WRAP",
    "TAKE DOWN",
    "THRASH",
    "DOUBLE-EDGE",
    "TAIL WHIP",
    "POISON STING",
    "TWINEEDLE",
    "PIN MISSILE",
    "LEER",
    "BITE",
    "GROWL",
    "ROAR",
    "SING",
    "SUPERSONIC",
    "SONICBOOM",
    "DISABLE",

    // Moves 051-075
    "ACID",
    "EMBER",
    "FLAMETHROWER",
    "MIST",
    "WATER GUN",
    "HYDRO PUMP",
    "SURF",
    "ICE BEAM",
    "BLIZZARD",
    "PSYBEAM",
    "BUBBLEBEAM",
    "AURORA BEAM",
    "HYPER BEAM",
    "PECK",
    "DRILL PECK",
    "SUBMISSION",
    "LOW KICK",
    "COUNTER",
    "SEISMIC TOSS",
    "STRENGTH",
    "ABSORB",
    "MEGA DRAIN",
    "LEECH SEED",
    "GROWTH",
    "RAZOR LEAF",

    // Moves 076-100
    "SOLARBEAM",
    "POISONPOWDER",
    "STUN SPORE",
    "SLEEP POWDER",
    "PETAL DANCE",
    "STRING SHOT",
    "DRAGON RAGE",
    "FIRE SPIN",
    "THUNDERSHOCK",
    "THUNDERBOLT",
    "THUNDER WAVE",
    "THUNDER",
    "ROCK THROW",
    "EARTHQUAKE",
    "FISSURE",
    "DIG",
    "TOXIC",
    "CONFUSION",
    "PSYCHIC",
    "HYPNOSIS",
    "MEDITATE",
    "AGILITY",
    "QUICK ATTACK",
    "RAGE",
    "TELEPORT",

    // Moves 101-125
    "NIGHT SHADE",
    "MIMIC",
    "SCREECH",
    "DOUBLE TEAM",
    "RECOVER",
    "HARDEN",
    "MINIMIZE",
    "SMOKESCREEN",
    "CONFUSE RAY",
    "WITHDRAW",
    "DEFENSE CURL",
    "BARRIER",
    "LIGHT SCREEN",
    "HAZE",
    "REFLECT",
    "FOCUS ENERGY",
    "BIDE",
    "METRONOME",
    "MIRROR MOVE",
    "SELFDESTRUCT",
    "EGG BOMB",
    "LICK",
    "SMOG",
    "SLUDGE",
    "BONE CLUB",

    // Moves 126-150
    "FIRE BLAST",
    "WATERFALL",
    "CLAMP",
    "SWIFT",
    "SKULL BASH",
    "SPIKE CANNON",
    "CONSTRICT",
    "AMNESIA",
    "KINESIS",
    "SOFTBOILED",
    "HI JUMP KICK",
    "GLARE",
    "DREAM EATER",
    "POISON GAS",
    "BARRAGE",
    "LEECH LIFE",
    "LOVELY KISS",
    "SKY ATTACK",
    "TRANSFORM",
    "BUBBLE",
    "DIZZY PUNCH",
    "SPORE",
    "FLASH",
    "PSYWAVE",
    "SPLASH",

    // Moves 151-175
    "ACID ARMOR",
    "CRABHAMMER",
    "EXPLOSION",
    "FURY SWIPES",
    "BONEMERANG",
    "REST",
    "ROCK SLIDE",
    "HYPER FANG",
    "SHARPEN",
    "CONVERSION",
    "TRI ATTACK",
    "SUPER FANG",
    "SLASH",
    "SUBSTITUTE",
    "STRUGGLE",
    "SKETCH",
    "TRIPLE KICK",
    "THIEF",
    "SPIDER WEB",
    "MIND READER",
    "NIGHTMARE",
    "FLAME WHEEL",
    "SNORE",
    "CURSE",
    "FLAIL",

    // Moves 176-200
    "CONVERSION 2",
    "AEROBLAST",
    "COTTON SPORE",
    "REVERSAL",
    "SPITE",
    "POWDER SNOW",
    "PROTECT",
    "MACH PUNCH",
    "SCARY FACE",
    "FAINT ATTACK",
    "SWEET KISS",
    "BELLY DRUM",
    "SLUDGE BOMB",
    "MUD-SLAP",
    "OCTAZOOKA",
    "SPIKES",
    "ZAP CANNON",
    "FORESIGHT",
    "DESTINY BOND",
    "PERISH SONG",
    "ICY WIND",
    "DETECT",
    "BONE RUSH",
    "LOCK-ON",
    "OUTRAGE",

    // Moves 201-225
    "SANDSTORM",
    "GIGA DRAIN",
    "ENDURE",
    "CHARM",
    "ROLLOUT",
    "FALSE SWIPE",
    "SWAGGER",
    "MILK DRINK",
    "SPARK",
    "FURY CUTTER",
    "STEEL WING",
    "MEAN LOOK",
    "ATTRACT",
    "SLEEP TALK",
    "HEAL BELL",
    "RETURN",
    "PRESENT",
    "FRUSTRATION",
    "SAFEGUARD",
    "PAIN SPLIT",
    "SACRED FIRE",
    "MAGNITUDE",
    "DYNAMICPUNCH",
    "MEGAHORN",
    "DRAGONBREATH",

    // Moves 226-251
    "BATON PASS",
    "ENCORE",
    "PURSUIT",
    "RAPID SPIN",
    "SWEET SCENT",
    "IRON TAIL",
    "METAL CLAW",
    "VITAL THROW",
    "MORNING SUN",
    "SYNTHESIS",
    "MOONLIGHT",
    "HIDDEN POWER",
    "CROSS CHOP",
    "TWISTER",
    "RAIN DANCE",
    "SUNNY DAY",
    "CRUNCH",
    "MIRROR COAT",
    "PSYCH UP",
    "EXTREMESPEED",
    "ANCIENTPOWER",
    "SHADOW BALL",
    "FUTURE SIGHT",
    "ROCK SMASH",
    "WHIRLPOOL",
    "BEAT UP",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_move_name() {
        assert_eq!(get_move_name(1), "POUND");
        assert_eq!(get_move_name(10), "SCRATCH");
        assert_eq!(get_move_name(33), "TACKLE");
        assert_eq!(get_move_name(43), "LEER");
        assert_eq!(get_move_name(99), "RAGE");
        assert_eq!(get_move_name(251), "BEAT UP");
    }

    #[test]
    fn test_invalid_move() {
        assert_eq!(get_move_name(0), "------");
        assert_eq!(get_move_name(252), "------");
    }

    #[test]
    fn test_name_lengths() {
        for name in MOVE_NAMES.iter() {
            assert!(name.len() <= MAX_MOVE_NAME_LEN, "{} is too long", name);
        }
    }
}

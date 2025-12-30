//! Pokemon Move PP (Power Points) Data
//!
//! Contains base PP values for all 251 moves from Generation 1 and 2.
//! Move ID 0 = no move, 1-251 are valid moves.
//!
//! PP values can be increased by PP Up items:
//! - Each PP Up adds 20% of base PP (up to 3 times)
//! - Max PP = Base PP * 1.6 (rounded down)
//!
//! The actual current PP is stored in the lower 6 bits of the PP byte.
//! The upper 2 bits store the number of PP Ups used (0-3).

/// Get base PP for a move by move ID (1-251)
/// Returns 0 for invalid/empty move slots
pub fn get_base_pp(move_id: u8) -> u8 {
    if move_id == 0 || move_id as usize > MOVE_BASE_PP.len() {
        return 0;
    }
    MOVE_BASE_PP[move_id as usize - 1]
}

/// Get max PP for a move (base PP * 1.6, rounded down)
/// This is the max after 3 PP Ups
pub fn get_max_pp(move_id: u8) -> u8 {
    let base = get_base_pp(move_id) as u16;
    ((base * 8) / 5) as u8  // * 1.6
}

/// Calculate actual max PP based on PP Ups used
/// pp_ups: 0-3 (number of PP Ups used)
pub fn get_actual_max_pp(move_id: u8, pp_ups: u8) -> u8 {
    let base = get_base_pp(move_id) as u16;
    let bonus = (base * (pp_ups.min(3) as u16)) / 5;  // Each PP Up = 20% = 1/5
    (base + bonus) as u8
}

/// Extract PP Ups count from raw PP byte
/// Upper 2 bits store the count (0-3)
pub fn extract_pp_ups(raw_pp: u8) -> u8 {
    (raw_pp >> 6) & 0x03
}

/// Extract current PP from raw PP byte
/// Lower 6 bits store the current PP (0-63)
pub fn extract_current_pp(raw_pp: u8) -> u8 {
    raw_pp & 0x3F
}

/// Base PP for all 251 moves (index 0 = move 1, etc.)
/// Values from Gen 2 (Gen 1 values are identical where applicable)
static MOVE_BASE_PP: [u8; 251] = [
    // Moves 001-025
    35,  // POUND
    25,  // KARATE CHOP
    10,  // DOUBLESLAP
    15,  // COMET PUNCH
    20,  // MEGA PUNCH
    20,  // PAY DAY
    15,  // FIRE PUNCH
    15,  // ICE PUNCH
    15,  // THUNDERPUNCH
    35,  // SCRATCH
    30,  // VICEGRIP
    5,   // GUILLOTINE
    10,  // RAZOR WIND
    30,  // SWORDS DANCE
    30,  // CUT
    35,  // GUST
    35,  // WING ATTACK
    20,  // WHIRLWIND
    15,  // FLY
    20,  // BIND
    20,  // SLAM
    15,  // VINE WHIP (was 10 in Gen 1)
    20,  // STOMP
    30,  // DOUBLE KICK
    5,   // MEGA KICK

    // Moves 026-050
    25,  // JUMP KICK (was 10 in Gen 1)
    15,  // ROLLING KICK
    15,  // SAND-ATTACK
    15,  // HEADBUTT
    25,  // HORN ATTACK
    20,  // FURY ATTACK
    5,   // HORN DRILL
    35,  // TACKLE
    15,  // BODY SLAM
    20,  // WRAP
    20,  // TAKE DOWN
    20,  // THRASH (was 10 in Gen 1)
    15,  // DOUBLE-EDGE
    30,  // TAIL WHIP
    35,  // POISON STING
    20,  // TWINEEDLE
    20,  // PIN MISSILE
    30,  // LEER
    25,  // BITE
    40,  // GROWL
    20,  // ROAR
    15,  // SING
    20,  // SUPERSONIC
    20,  // SONICBOOM
    20,  // DISABLE

    // Moves 051-075
    30,  // ACID
    25,  // EMBER
    15,  // FLAMETHROWER
    30,  // MIST
    25,  // WATER GUN
    5,   // HYDRO PUMP
    15,  // SURF
    10,  // ICE BEAM
    5,   // BLIZZARD
    20,  // PSYBEAM
    20,  // BUBBLEBEAM
    20,  // AURORA BEAM
    5,   // HYPER BEAM
    35,  // PECK
    20,  // DRILL PECK
    25,  // SUBMISSION (was 20 in Gen 1)
    20,  // LOW KICK
    20,  // COUNTER
    20,  // SEISMIC TOSS
    15,  // STRENGTH
    20,  // ABSORB (was 25 in Gen 1)
    10,  // MEGA DRAIN (was 15 in Gen 1)
    10,  // LEECH SEED
    40,  // GROWTH
    25,  // RAZOR LEAF

    // Moves 076-100
    10,  // SOLARBEAM
    35,  // POISONPOWDER
    30,  // STUN SPORE
    15,  // SLEEP POWDER
    20,  // PETAL DANCE (was 10 in Gen 1)
    40,  // STRING SHOT
    10,  // DRAGON RAGE
    15,  // FIRE SPIN
    30,  // THUNDERSHOCK
    15,  // THUNDERBOLT
    20,  // THUNDER WAVE
    10,  // THUNDER
    20,  // ROCK THROW
    10,  // EARTHQUAKE
    5,   // FISSURE
    10,  // DIG
    10,  // TOXIC
    25,  // CONFUSION
    10,  // PSYCHIC
    20,  // HYPNOSIS
    40,  // MEDITATE
    30,  // AGILITY
    30,  // QUICK ATTACK
    20,  // RAGE

    // Moves 101-125
    20,  // TELEPORT
    15,  // NIGHT SHADE
    10,  // MIMIC
    40,  // SCREECH
    15,  // DOUBLE TEAM
    20,  // RECOVER (was 10 in Gen 1)
    30,  // HARDEN
    20,  // MINIMIZE
    20,  // SMOKESCREEN
    10,  // CONFUSE RAY
    40,  // WITHDRAW
    40,  // DEFENSE CURL
    30,  // BARRIER
    30,  // LIGHT SCREEN
    20,  // HAZE
    30,  // REFLECT
    30,  // FOCUS ENERGY
    10,  // BIDE
    10,  // METRONOME
    20,  // MIRROR MOVE
    5,   // SELFDESTRUCT
    10,  // EGG BOMB
    30,  // LICK
    20,  // SMOG
    20,  // SLUDGE

    // Moves 126-150
    20,  // BONE CLUB
    5,   // FIRE BLAST
    15,  // WATERFALL
    10,  // CLAMP
    20,  // SWIFT
    15,  // SKULL BASH
    10,  // SPIKE CANNON
    20,  // CONSTRICT
    20,  // AMNESIA
    15,  // KINESIS
    10,  // SOFTBOILED (was 5 in Gen 1)
    20,  // HI JUMP KICK (was 10 in Gen 1)
    30,  // GLARE
    15,  // DREAM EATER
    40,  // POISON GAS
    20,  // BARRAGE
    15,  // LEECH LIFE (was 10 in Gen 1)
    10,  // LOVELY KISS
    5,   // SKY ATTACK
    10,  // TRANSFORM
    30,  // BUBBLE
    10,  // DIZZY PUNCH
    15,  // SPORE
    20,  // FLASH
    15,  // PSYWAVE

    // Moves 151-175
    40,  // SPLASH
    20,  // ACID ARMOR
    10,  // CRABHAMMER
    5,   // EXPLOSION
    15,  // FURY SWIPES
    10,  // BONEMERANG
    10,  // REST
    10,  // ROCK SLIDE
    15,  // HYPER FANG
    30,  // SHARPEN
    30,  // CONVERSION
    10,  // TRI ATTACK
    10,  // SUPER FANG
    20,  // SLASH
    10,  // SUBSTITUTE
    5,   // STRUGGLE
    1,   // SKETCH
    10,  // TRIPLE KICK
    10,  // THIEF
    10,  // SPIDER WEB
    5,   // MIND READER
    15,  // NIGHTMARE
    25,  // FLAME WHEEL
    15,  // SNORE
    10,  // CURSE

    // Moves 176-200
    30,  // FLAIL
    30,  // CONVERSION 2
    5,   // AEROBLAST
    40,  // COTTON SPORE
    15,  // REVERSAL
    10,  // SPITE
    25,  // POWDER SNOW
    10,  // PROTECT
    30,  // MACH PUNCH
    10,  // SCARY FACE
    20,  // FAINT ATTACK
    10,  // SWEET KISS
    10,  // BELLY DRUM
    10,  // SLUDGE BOMB
    10,  // MUD-SLAP
    10,  // OCTAZOOKA
    20,  // SPIKES
    5,   // ZAP CANNON
    40,  // FORESIGHT
    5,   // DESTINY BOND
    5,   // PERISH SONG
    15,  // ICY WIND
    5,   // DETECT
    10,  // BONE RUSH
    5,   // LOCK-ON

    // Moves 201-225
    5,   // OUTRAGE
    10,  // SANDSTORM
    5,   // GIGA DRAIN
    10,  // ENDURE
    20,  // CHARM
    20,  // ROLLOUT
    15,  // FALSE SWIPE
    15,  // SWAGGER
    10,  // MILK DRINK
    20,  // SPARK
    20,  // FURY CUTTER
    25,  // STEEL WING
    5,   // MEAN LOOK
    15,  // ATTRACT
    10,  // SLEEP TALK
    20,  // HEAL BELL
    15,  // RETURN
    15,  // PRESENT
    15,  // FRUSTRATION
    25,  // SAFEGUARD
    20,  // PAIN SPLIT
    5,   // SACRED FIRE
    5,   // MAGNITUDE
    5,   // DYNAMICPUNCH
    10,  // MEGAHORN

    // Moves 226-251
    20,  // DRAGONBREATH
    40,  // BATON PASS
    5,   // ENCORE
    20,  // PURSUIT
    40,  // RAPID SPIN
    20,  // SWEET SCENT
    15,  // IRON TAIL
    35,  // METAL CLAW
    10,  // VITAL THROW
    5,   // MORNING SUN
    5,   // SYNTHESIS
    5,   // MOONLIGHT
    15,  // HIDDEN POWER
    5,   // CROSS CHOP
    20,  // TWISTER
    5,   // RAIN DANCE
    5,   // SUNNY DAY
    15,  // CRUNCH
    20,  // MIRROR COAT
    5,   // PSYCH UP
    5,   // EXTREMESPEED
    5,   // ANCIENTPOWER
    15,  // SHADOW BALL
    15,  // FUTURE SIGHT
    15,  // ROCK SMASH
    15,  // WHIRLPOOL
    10,  // BEAT UP
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_pp() {
        assert_eq!(get_base_pp(1), 35);   // POUND
        assert_eq!(get_base_pp(33), 35);  // TACKLE
        assert_eq!(get_base_pp(63), 5);   // HYPER BEAM
        assert_eq!(get_base_pp(0), 0);    // Invalid
        assert_eq!(get_base_pp(252), 0);  // Invalid
    }

    #[test]
    fn test_max_pp() {
        assert_eq!(get_max_pp(1), 56);    // POUND: 35 * 1.6 = 56
        assert_eq!(get_max_pp(33), 56);   // TACKLE: 35 * 1.6 = 56
        assert_eq!(get_max_pp(63), 8);    // HYPER BEAM: 5 * 1.6 = 8
    }

    #[test]
    fn test_actual_max_pp() {
        // POUND (base 35)
        assert_eq!(get_actual_max_pp(1, 0), 35);  // 0 PP Ups
        assert_eq!(get_actual_max_pp(1, 1), 42);  // 1 PP Up: 35 + 7
        assert_eq!(get_actual_max_pp(1, 2), 49);  // 2 PP Ups: 35 + 14
        assert_eq!(get_actual_max_pp(1, 3), 56);  // 3 PP Ups: 35 + 21
    }

    #[test]
    fn test_extract_pp() {
        // PP byte: upper 2 bits = PP ups, lower 6 bits = current PP
        assert_eq!(extract_pp_ups(0b00_111111), 0);
        assert_eq!(extract_pp_ups(0b01_111111), 1);
        assert_eq!(extract_pp_ups(0b10_111111), 2);
        assert_eq!(extract_pp_ups(0b11_111111), 3);

        assert_eq!(extract_current_pp(0b11_100011), 35);
        assert_eq!(extract_current_pp(0b00_000101), 5);
    }
}

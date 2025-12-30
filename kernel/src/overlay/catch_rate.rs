//! Pokemon Catch Rate Data
//!
//! Contains catch rates for all 251 Pokemon species.
//! Catch rate affects the probability of catching a wild Pokemon.
//! Higher values = easier to catch (max 255).
//!
//! Formula: Catch Rate ≈ 3 * max_hp * rate / (3 * max_hp - 2 * current_hp)
//! Then modified by ball type and status conditions.

/// Get catch rate for a species (1-251)
/// Returns 0 for invalid species IDs
pub fn get_catch_rate(species: u8) -> u8 {
    if species == 0 || species as usize > CATCH_RATES.len() {
        return 0;
    }
    CATCH_RATES[species as usize - 1]
}

/// Get catch rate as a percentage estimate (rough approximation)
/// This is a simplified estimate assuming full HP, no status, Poke Ball
pub fn get_catch_percent(species: u8) -> u8 {
    let rate = get_catch_rate(species) as u16;
    // Rough estimate: rate/255 * 100 / 3 (because full HP)
    ((rate * 100) / (255 * 3)).min(100) as u8
}

/// Catch difficulty tier for display
pub fn get_catch_tier(species: u8) -> &'static str {
    let rate = get_catch_rate(species);
    match rate {
        0 => "???",
        1..=3 => "LEGENDARY",
        4..=15 => "VERY HARD",
        16..=45 => "HARD",
        46..=100 => "MEDIUM",
        101..=190 => "EASY",
        191..=255 => "VERY EASY",
    }
}

/// Catch rates for all 251 Pokemon (index 0 = species 1, etc.)
/// Values from Gen 2 (some changed from Gen 1)
static CATCH_RATES: [u8; 251] = [
    // Gen 1: 001-025 (Bulbasaur line through Pikachu)
    45,   // Bulbasaur
    45,   // Ivysaur
    45,   // Venusaur
    45,   // Charmander
    45,   // Charmeleon
    45,   // Charizard
    45,   // Squirtle
    45,   // Wartortle
    45,   // Blastoise
    255,  // Caterpie
    120,  // Metapod
    45,   // Butterfree
    255,  // Weedle
    120,  // Kakuna
    45,   // Beedrill
    255,  // Pidgey
    120,  // Pidgeotto
    45,   // Pidgeot
    255,  // Rattata
    127,  // Raticate
    255,  // Spearow
    90,   // Fearow
    255,  // Ekans
    90,   // Arbok
    190,  // Pikachu

    // Gen 1: 026-050 (Raichu through Diglett)
    75,   // Raichu
    255,  // Sandshrew
    90,   // Sandslash
    235,  // Nidoran♀
    120,  // Nidorina
    45,   // Nidoqueen
    235,  // Nidoran♂
    120,  // Nidorino
    45,   // Nidoking
    150,  // Clefairy
    25,   // Clefable
    190,  // Vulpix
    75,   // Ninetales
    170,  // Jigglypuff
    50,   // Wigglytuff
    255,  // Zubat
    90,   // Golbat
    255,  // Oddish
    120,  // Gloom
    45,   // Vileplume
    190,  // Paras
    75,   // Parasect
    190,  // Venonat
    75,   // Venomoth
    255,  // Diglett

    // Gen 1: 051-075 (Dugtrio through Graveler)
    50,   // Dugtrio
    255,  // Meowth
    90,   // Persian
    190,  // Psyduck
    75,   // Golduck
    190,  // Mankey
    75,   // Primeape
    190,  // Growlithe
    75,   // Arcanine
    255,  // Poliwag
    120,  // Poliwhirl
    45,   // Poliwrath
    200,  // Abra
    100,  // Kadabra
    50,   // Alakazam
    180,  // Machop
    90,   // Machoke
    45,   // Machamp
    255,  // Bellsprout
    120,  // Weepinbell
    45,   // Victreebel
    190,  // Tentacool
    60,   // Tentacruel
    255,  // Geodude
    120,  // Graveler

    // Gen 1: 076-100 (Golem through Voltorb)
    45,   // Golem
    190,  // Ponyta
    60,   // Rapidash
    190,  // Slowpoke
    75,   // Slowbro
    190,  // Magnemite
    60,   // Magneton
    45,   // Farfetch'd
    190,  // Doduo
    45,   // Dodrio
    190,  // Seel
    75,   // Dewgong
    190,  // Grimer
    75,   // Muk
    190,  // Shellder
    60,   // Cloyster
    190,  // Gastly
    90,   // Haunter
    45,   // Gengar
    45,   // Onix
    190,  // Drowzee
    75,   // Hypno
    225,  // Krabby
    60,   // Kingler
    190,  // Voltorb

    // Gen 1: 101-125 (Electrode through Electabuzz)
    60,   // Electrode
    90,   // Exeggcute
    45,   // Exeggutor
    190,  // Cubone
    75,   // Marowak
    45,   // Hitmonlee
    45,   // Hitmonchan
    45,   // Lickitung
    190,  // Koffing
    60,   // Weezing
    120,  // Rhyhorn
    60,   // Rhydon
    30,   // Chansey
    45,   // Tangela
    45,   // Kangaskhan
    225,  // Horsea
    75,   // Seadra
    225,  // Goldeen
    60,   // Seaking
    225,  // Staryu
    60,   // Starmie
    45,   // Mr. Mime
    45,   // Scyther
    45,   // Jynx
    45,   // Electabuzz

    // Gen 1: 126-151 (Magmar through Mew)
    45,   // Magmar
    45,   // Pinsir
    45,   // Tauros
    255,  // Magikarp
    45,   // Gyarados
    45,   // Lapras
    35,   // Ditto
    45,   // Eevee
    45,   // Vaporeon
    45,   // Jolteon
    45,   // Flareon
    45,   // Porygon
    45,   // Omanyte
    45,   // Omastar
    45,   // Kabuto
    45,   // Kabutops
    45,   // Aerodactyl
    25,   // Snorlax
    3,    // Articuno
    3,    // Zapdos
    3,    // Moltres
    45,   // Dratini
    45,   // Dragonair
    45,   // Dragonite
    3,    // Mewtwo
    45,   // Mew

    // Gen 2: 152-175 (Chikorita through Togepi)
    45,   // Chikorita
    45,   // Bayleef
    45,   // Meganium
    45,   // Cyndaquil
    45,   // Quilava
    45,   // Typhlosion
    45,   // Totodile
    45,   // Croconaw
    45,   // Feraligatr
    255,  // Sentret
    90,   // Furret
    255,  // Hoothoot
    90,   // Noctowl
    255,  // Ledyba
    90,   // Ledian
    255,  // Spinarak
    90,   // Ariados
    90,   // Crobat
    190,  // Chinchou
    75,   // Lanturn
    190,  // Pichu
    150,  // Cleffa
    170,  // Igglybuff
    190,  // Togepi

    // Gen 2: 176-200 (Togetic through Misdreavus)
    75,   // Togetic
    190,  // Natu
    75,   // Xatu
    235,  // Mareep
    120,  // Flaaffy
    45,   // Ampharos
    45,   // Bellossom
    190,  // Marill
    75,   // Azumarill
    65,   // Sudowoodo
    45,   // Politoed
    255,  // Hoppip
    120,  // Skiploom
    45,   // Jumpluff
    45,   // Aipom
    235,  // Sunkern
    120,  // Sunflora
    75,   // Yanma
    255,  // Wooper
    90,   // Quagsire
    45,   // Espeon
    45,   // Umbreon
    30,   // Murkrow
    70,   // Slowking
    45,   // Misdreavus

    // Gen 2: 201-225 (Unown through Delibird)
    225,  // Unown
    45,   // Wobbuffet
    60,   // Girafarig
    190,  // Pineco
    75,   // Forretress
    190,  // Dunsparce
    60,   // Gligar
    25,   // Steelix
    190,  // Snubbull
    75,   // Granbull
    45,   // Qwilfish
    25,   // Scizor
    190,  // Shuckle
    45,   // Heracross
    60,   // Sneasel
    120,  // Teddiursa
    60,   // Ursaring
    190,  // Slugma
    75,   // Magcargo
    225,  // Swinub
    75,   // Piloswine
    60,   // Corsola
    190,  // Remoraid
    75,   // Octillery
    45,   // Delibird

    // Gen 2: 226-251 (Mantine through Celebi)
    25,   // Mantine
    25,   // Skarmory
    120,  // Houndour
    45,   // Houndoom
    45,   // Kingdra
    120,  // Phanpy
    60,   // Donphan
    45,   // Porygon2
    45,   // Stantler
    45,   // Smeargle
    75,   // Tyrogue
    45,   // Hitmontop
    45,   // Smoochum
    45,   // Elekid
    45,   // Magby
    45,   // Miltank
    30,   // Blissey
    3,    // Raikou
    3,    // Entei
    3,    // Suicune
    45,   // Larvitar
    45,   // Pupitar
    45,   // Tyranitar
    3,    // Lugia
    3,    // Ho-Oh
    45,   // Celebi
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catch_rates() {
        assert_eq!(get_catch_rate(1), 45);    // Bulbasaur
        assert_eq!(get_catch_rate(10), 255);  // Caterpie
        assert_eq!(get_catch_rate(129), 255); // Magikarp
        assert_eq!(get_catch_rate(150), 3);   // Mewtwo
        assert_eq!(get_catch_rate(151), 45);  // Mew
        assert_eq!(get_catch_rate(249), 3);   // Lugia
    }

    #[test]
    fn test_catch_tier() {
        assert_eq!(get_catch_tier(150), "LEGENDARY");  // Mewtwo (3)
        assert_eq!(get_catch_tier(1), "HARD");         // Bulbasaur (45)
        assert_eq!(get_catch_tier(129), "VERY EASY");  // Magikarp (255)
        assert_eq!(get_catch_tier(0), "???");          // Invalid
    }

    #[test]
    fn test_invalid_species() {
        assert_eq!(get_catch_rate(0), 0);
        assert_eq!(get_catch_rate(252), 0);
    }
}

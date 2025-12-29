//! Pokemon Name Lookup Table
//!
//! Contains names for all 251 Pokemon from Generation 1 and 2.
//! Species IDs 1-151 are Gen 1, 152-251 are Gen 2.

/// Maximum Pokemon name length (10 characters + null terminator space)
pub const MAX_NAME_LEN: usize = 10;

/// Get Pokemon name by species ID (1-251)
/// Returns "??????" for invalid IDs
pub fn get_name(species: u8) -> &'static str {
    if species == 0 || species as usize > POKEMON_NAMES.len() {
        return "??????";
    }
    POKEMON_NAMES[species as usize - 1]
}

/// Get Pokemon name as bytes for direct rendering
pub fn get_name_bytes(species: u8) -> &'static [u8] {
    get_name(species).as_bytes()
}

/// All 251 Pokemon names (index 0 = species 1, etc.)
static POKEMON_NAMES: [&str; 251] = [
    // Gen 1: 001-025
    "BULBASAUR",
    "IVYSAUR",
    "VENUSAUR",
    "CHARMANDER",
    "CHARMELEON",
    "CHARIZARD",
    "SQUIRTLE",
    "WARTORTLE",
    "BLASTOISE",
    "CATERPIE",
    "METAPOD",
    "BUTTERFREE",
    "WEEDLE",
    "KAKUNA",
    "BEEDRILL",
    "PIDGEY",
    "PIDGEOTTO",
    "PIDGEOT",
    "RATTATA",
    "RATICATE",
    "SPEAROW",
    "FEAROW",
    "EKANS",
    "ARBOK",
    "PIKACHU",

    // Gen 1: 026-050
    "RAICHU",
    "SANDSHREW",
    "SANDSLASH",
    "NIDORAN F",  // ♀
    "NIDORINA",
    "NIDOQUEEN",
    "NIDORAN M",  // ♂
    "NIDORINO",
    "NIDOKING",
    "CLEFAIRY",
    "CLEFABLE",
    "VULPIX",
    "NINETALES",
    "JIGGLYPUFF",
    "WIGGLYTUFF",
    "ZUBAT",
    "GOLBAT",
    "ODDISH",
    "GLOOM",
    "VILEPLUME",
    "PARAS",
    "PARASECT",
    "VENONAT",
    "VENOMOTH",
    "DIGLETT",

    // Gen 1: 051-075
    "DUGTRIO",
    "MEOWTH",
    "PERSIAN",
    "PSYDUCK",
    "GOLDUCK",
    "MANKEY",
    "PRIMEAPE",
    "GROWLITHE",
    "ARCANINE",
    "POLIWAG",
    "POLIWHIRL",
    "POLIWRATH",
    "ABRA",
    "KADABRA",
    "ALAKAZAM",
    "MACHOP",
    "MACHOKE",
    "MACHAMP",
    "BELLSPROUT",
    "WEEPINBELL",
    "VICTREEBEL",
    "TENTACOOL",
    "TENTACRUEL",
    "GEODUDE",
    "GRAVELER",

    // Gen 1: 076-100
    "GOLEM",
    "PONYTA",
    "RAPIDASH",
    "SLOWPOKE",
    "SLOWBRO",
    "MAGNEMITE",
    "MAGNETON",
    "FARFETCH'D",
    "DODUO",
    "DODRIO",
    "SEEL",
    "DEWGONG",
    "GRIMER",
    "MUK",
    "SHELLDER",
    "CLOYSTER",
    "GASTLY",
    "HAUNTER",
    "GENGAR",
    "ONIX",
    "DROWZEE",
    "HYPNO",
    "KRABBY",
    "KINGLER",
    "VOLTORB",

    // Gen 1: 101-125
    "ELECTRODE",
    "EXEGGCUTE",
    "EXEGGUTOR",
    "CUBONE",
    "MAROWAK",
    "HITMONLEE",
    "HITMONCHAN",
    "LICKITUNG",
    "KOFFING",
    "WEEZING",
    "RHYHORN",
    "RHYDON",
    "CHANSEY",
    "TANGELA",
    "KANGASKHAN",
    "HORSEA",
    "SEADRA",
    "GOLDEEN",
    "SEAKING",
    "STARYU",
    "STARMIE",
    "MR.MIME",
    "SCYTHER",
    "JYNX",
    "ELECTABUZZ",

    // Gen 1: 126-151
    "MAGMAR",
    "PINSIR",
    "TAUROS",
    "MAGIKARP",
    "GYARADOS",
    "LAPRAS",
    "DITTO",
    "EEVEE",
    "VAPOREON",
    "JOLTEON",
    "FLAREON",
    "PORYGON",
    "OMANYTE",
    "OMASTAR",
    "KABUTO",
    "KABUTOPS",
    "AERODACTYL",
    "SNORLAX",
    "ARTICUNO",
    "ZAPDOS",
    "MOLTRES",
    "DRATINI",
    "DRAGONAIR",
    "DRAGONITE",
    "MEWTWO",
    "MEW",

    // Gen 2: 152-175
    "CHIKORITA",
    "BAYLEEF",
    "MEGANIUM",
    "CYNDAQUIL",
    "QUILAVA",
    "TYPHLOSION",
    "TOTODILE",
    "CROCONAW",
    "FERALIGATR",
    "SENTRET",
    "FURRET",
    "HOOTHOOT",
    "NOCTOWL",
    "LEDYBA",
    "LEDIAN",
    "SPINARAK",
    "ARIADOS",
    "CROBAT",
    "CHINCHOU",
    "LANTURN",
    "PICHU",
    "CLEFFA",
    "IGGLYBUFF",
    "TOGEPI",

    // Gen 2: 176-200
    "TOGETIC",
    "NATU",
    "XATU",
    "MAREEP",
    "FLAAFFY",
    "AMPHAROS",
    "BELLOSSOM",
    "MARILL",
    "AZUMARILL",
    "SUDOWOODO",
    "POLITOED",
    "HOPPIP",
    "SKIPLOOM",
    "JUMPLUFF",
    "AIPOM",
    "SUNKERN",
    "SUNFLORA",
    "YANMA",
    "WOOPER",
    "QUAGSIRE",
    "ESPEON",
    "UMBREON",
    "MURKROW",
    "SLOWKING",
    "MISDREAVUS",

    // Gen 2: 201-225
    "UNOWN",
    "WOBBUFFET",
    "GIRAFARIG",
    "PINECO",
    "FORRETRESS",
    "DUNSPARCE",
    "GLIGAR",
    "STEELIX",
    "SNUBBULL",
    "GRANBULL",
    "QWILFISH",
    "SCIZOR",
    "SHUCKLE",
    "HERACROSS",
    "SNEASEL",
    "TEDDIURSA",
    "URSARING",
    "SLUGMA",
    "MAGCARGO",
    "SWINUB",
    "PILOSWINE",
    "CORSOLA",
    "REMORAID",
    "OCTILLERY",
    "DELIBIRD",

    // Gen 2: 226-251
    "MANTINE",
    "SKARMORY",
    "HOUNDOUR",
    "HOUNDOOM",
    "KINGDRA",
    "PHANPY",
    "DONPHAN",
    "PORYGON2",
    "STANTLER",
    "SMEARGLE",
    "TYROGUE",
    "HITMONTOP",
    "SMOOCHUM",
    "ELEKID",
    "MAGBY",
    "MILTANK",
    "BLISSEY",
    "RAIKOU",
    "ENTEI",
    "SUICUNE",
    "LARVITAR",
    "PUPITAR",
    "TYRANITAR",
    "LUGIA",
    "HO-OH",
    "CELEBI",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_name() {
        assert_eq!(get_name(1), "BULBASAUR");
        assert_eq!(get_name(25), "PIKACHU");
        assert_eq!(get_name(151), "MEW");
        assert_eq!(get_name(152), "CHIKORITA");
        assert_eq!(get_name(158), "TOTODILE");
        assert_eq!(get_name(251), "CELEBI");
    }

    #[test]
    fn test_invalid_species() {
        assert_eq!(get_name(0), "??????");
        assert_eq!(get_name(252), "??????");
    }

    #[test]
    fn test_name_lengths() {
        for name in POKEMON_NAMES.iter() {
            assert!(name.len() <= MAX_NAME_LEN, "{} is too long", name);
        }
    }
}

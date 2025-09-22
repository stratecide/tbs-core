use zipper::*;

use crate::config::parse::FromConfig;
use crate::map::direction::Direction;
use crate::config::environment::Environment;
use super::unit::UnitBuilder;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitType(pub usize);

impl FromConfig for UnitType {
    fn from_conf<'a>(s: &'a str, loader: &mut crate::config::file_loader::FileLoader) -> Result<(Self, &'a str), crate::config::ConfigParseError> {
        let (base, s) = crate::config::parse::string_base(s);
        match loader.unit_types.iter().position(|name| name.as_str() == base) {
            Some(i) => Ok((Self(i), s)),
            None => Err(crate::config::ConfigParseError::MissingUnit(base.to_string()))
        }
    }
}

impl SupportedZippable<&Environment> for UnitType {
    fn export(&self, zipper: &mut Zipper, environment: &Environment) {
        let bits = bits_needed_for_max_value(environment.config.unit_count() as u32 - 1);
        zipper.write_u32(self.0 as u32, bits);
    }
    fn import(unzipper: &mut Unzipper, environment: &Environment) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(environment.config.unit_count() as u32 - 1);
        let index = unzipper.read_u32(bits)? as usize;
        if index >= environment.config.unit_count() {
            return Err(ZipperError::EnumOutOfBounds(format!("UnitType index {}", index)))
        }
        Ok(Self(index))
    }
}

impl UnitType {
    pub fn instance<D: Direction>(&self, environment: &Environment) -> UnitBuilder<D> {
        UnitBuilder::new(environment, *self)
    }
}

#[cfg(test)]
mod helper {
    use super::UnitType;

    impl UnitType {
        pub const MARINE: Self = Self(0);
        pub const SNIPER: Self = Self(1);
        pub const BAZOOKA: Self = Self(2);
        pub const MAGNET: Self = Self(3);
        pub const DRAGON_HEAD: Self = Self(4);
        pub const CONVOY: Self = Self(5);
        pub const ARTILLERY: Self = Self(6);
        pub const SMALL_TANK: Self = Self(7);
        pub const DRONE_BOAT: Self = Self(13);
        pub const WAVE_BREAKER: Self = Self(14);
        pub const DESTROYER: Self = Self(17);
        pub const WAR_SHIP: Self = Self(18);
        pub const TRANSPORT_HELI: Self = Self(21);
        pub const ATTACK_HELI: Self = Self(22);
        pub const LIGHT_DRONE: Self = Self(26);
        pub const FACTORY: Self = Self(27);
        pub const HERO_CRYSTAL: Self = Self(28);
        pub const PYRAMID: Self = Self(29);
        pub const LIFE_CRYSTAL: Self = Self(34);
        pub const TENTACLE: Self = Self(35);
        pub const PUFFER_FISH: Self = Self(36);
        pub const PAWN: Self = Self(37);
        pub const ROOK: Self = Self(38);
        pub const BISHOP: Self = Self(39);
        pub const KNIGHT: Self = Self(40);
        pub const QUEEN: Self = Self(41);
        pub const KING: Self = Self(42);
        pub const UNKNOWN: Self = Self(43);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::config::Config;

    #[test]
    fn helpers_are_correct() {
        let config = Config::default();
        assert_eq!(config.unit_name(UnitType::MARINE), "Marine");
        assert_eq!(config.unit_name(UnitType::SNIPER), "Sniper");
        assert_eq!(config.unit_name(UnitType::BAZOOKA), "Bazooka");
        assert_eq!(config.unit_name(UnitType::MAGNET), "Magnet");
        assert_eq!(config.unit_name(UnitType::DRAGON_HEAD), "DragonHead");
        assert_eq!(config.unit_name(UnitType::CONVOY), "Convoy");
        assert_eq!(config.unit_name(UnitType::ARTILLERY), "Artillery");
        assert_eq!(config.unit_name(UnitType::SMALL_TANK), "SmallTank");
        assert_eq!(config.unit_name(UnitType::DRONE_BOAT), "DroneBoat");
        assert_eq!(config.unit_name(UnitType::WAVE_BREAKER), "WaveBreaker");
        assert_eq!(config.unit_name(UnitType::DESTROYER), "Destroyer");
        assert_eq!(config.unit_name(UnitType::WAR_SHIP), "WarShip");
        assert_eq!(config.unit_name(UnitType::TRANSPORT_HELI), "TransportHeli");
        assert_eq!(config.unit_name(UnitType::ATTACK_HELI), "AttackHeli");
        assert_eq!(config.unit_name(UnitType::LIGHT_DRONE), "LightDrone");
        assert_eq!(config.unit_name(UnitType::FACTORY), "Factory");
        assert_eq!(config.unit_name(UnitType::HERO_CRYSTAL), "HeroCrystal");
        assert_eq!(config.unit_name(UnitType::PYRAMID), "Pyramid");
        assert_eq!(config.unit_name(UnitType::LIFE_CRYSTAL), "LifeCrystal");
        assert_eq!(config.unit_name(UnitType::TENTACLE), "Tentacle");
        assert_eq!(config.unit_name(UnitType::PUFFER_FISH), "PufferFish");
        assert_eq!(config.unit_name(UnitType::PAWN), "Pawn");
        assert_eq!(config.unit_name(UnitType::ROOK), "Rook");
        assert_eq!(config.unit_name(UnitType::BISHOP), "Bishop");
        assert_eq!(config.unit_name(UnitType::KNIGHT), "Knight");
        assert_eq!(config.unit_name(UnitType::QUEEN), "Queen");
        assert_eq!(config.unit_name(UnitType::KING), "King");
        assert_eq!(config.unit_name(UnitType::UNKNOWN), "Unknown");
    }
}

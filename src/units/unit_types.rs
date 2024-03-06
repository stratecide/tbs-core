use zipper::*;

use crate::map::direction::Direction;
use crate::config::environment::Environment;
use super::unit::UnitBuilder;

crate::enum_with_custom! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum UnitType {
        HoverBike,
        // ground units
        Sniper,
        Bazooka,
        Magnet,
        DragonHead,
        Artillery,
        SmallTank,
        BigTank,
        AntiAir,
        RocketLauncher,
        // sea units
        LaserShark,
        TransportBoat,
        DroneBoat,
        WaveBreaker,
        Submarine,
        Cruiser,
        Carrier,
        SwimmingFactory,
        // air units
        TransportHeli,
        AttackHeli,
        Blimp,
        Bomber,
        Fighter,
        // drones
        LightDrone,
        HeavyDrone,
        //structures
        Pyramid,
        MegaCannon,
        LaserCannon,
        DroneTower,
        ShockTower,
        LifeCrystal,
        Tentacle,
        HeroCrystal,
        // chess
        Pawn,
        Rook,
        Bishop,
        Knight,
        Queen,
        King,
            // question mark
        Unknown,
    }
}

impl UnitType {
    pub fn base_cost(&self, environment: &Environment) -> i32 {
        environment.config.base_cost(*self)
    }

    pub fn instance<D: Direction>(&self, environment: &Environment) -> UnitBuilder<D> {
        UnitBuilder::new(environment, *self)
    }
}

impl SupportedZippable<&Environment> for UnitType {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        let index = support.config.unit_types().iter().position(|t| t == self).unwrap();
        let bits = bits_needed_for_max_value(support.config.unit_count() as u32 - 1);
        zipper.write_u32(index as u32, bits);
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(support.config.unit_count() as u32 - 1);
        let index = unzipper.read_u32(bits)? as usize;
        if index < support.config.unit_count() {
            Ok(support.config.unit_types()[index])
        } else {
            Err(ZipperError::EnumOutOfBounds(format!("UnitType index {}", index)))
        }
    }
}

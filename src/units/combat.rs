use crate::{game::events::Effect, map::point::Point};



pub enum AttackType {
    None,
    Adjacent,
    Ranged(u8, u8),
    Straight(u8, u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WeaponType {
    MachineGun,
    Shells,
    AntiAir,
    Flame,
    Rocket,
    Torpedo,
    Rifle,
    Bombs,
    // immobile ranged
    SurfaceMissiles,
    AirMissiles,
}
impl WeaponType {
    pub fn damage_factor(&self, armor: &ArmorType) -> Option<f32> {
        match (self, armor) {
            (Self::MachineGun, ArmorType::Infantry) => Some(1.00),
            (Self::MachineGun, ArmorType::Light) => Some(0.25),
            (Self::MachineGun, ArmorType::Heavy) => Some(0.10),
            (Self::MachineGun, ArmorType::Heli) => Some(0.20),
            (Self::MachineGun, ArmorType::Plane) => None,
            (Self::MachineGun, ArmorType::Boat) => None,
            (Self::MachineGun, ArmorType::Ship) => None,
            (Self::MachineGun, ArmorType::Submarine) => None,
            (Self::MachineGun, ArmorType::Structure) => Some(0.20),

            (Self::Rifle, ArmorType::Infantry) => Some(1.20),
            (Self::Rifle, ArmorType::Light) => Some(0.35),
            (Self::Rifle, ArmorType::Heavy) => Some(0.05),
            (Self::Rifle, ArmorType::Heli) => Some(0.30),
            (Self::Rifle, ArmorType::Plane) => Some(0.15),
            (Self::Rifle, ArmorType::Boat) => Some(0.10),
            (Self::Rifle, ArmorType::Ship) => None,
            (Self::Rifle, ArmorType::Submarine) => None,
            (Self::Rifle, ArmorType::Structure) => Some(0.10),

            (Self::Shells, ArmorType::Infantry) => Some(0.90),
            (Self::Shells, ArmorType::Light) => Some(1.10),
            (Self::Shells, ArmorType::Heavy) => Some(1.00),
            (Self::Shells, ArmorType::Heli) => None,
            (Self::Shells, ArmorType::Plane) => None,
            (Self::Shells, ArmorType::Boat) => Some(0.30),
            (Self::Shells, ArmorType::Ship) => Some(0.10),
            (Self::Shells, ArmorType::Submarine) => Some(1.00),
            (Self::Shells, ArmorType::Structure) => Some(1.00),

            (Self::Bombs, ArmorType::Infantry) => Some(1.10),
            (Self::Bombs, ArmorType::Light) => Some(1.10),
            (Self::Bombs, ArmorType::Heavy) => Some(0.9),
            (Self::Bombs, ArmorType::Heli) => None,
            (Self::Bombs, ArmorType::Plane) => None,
            (Self::Bombs, ArmorType::Boat) => Some(1.10),
            (Self::Bombs, ArmorType::Ship) => Some(0.9),
            (Self::Bombs, ArmorType::Submarine) => Some(0.80),
            (Self::Bombs, ArmorType::Structure) => Some(1.00),

            (Self::AntiAir, ArmorType::Heli) => Some(1.50),
            (Self::AntiAir, ArmorType::Plane) => Some(1.20),
            (Self::AntiAir, _) => None,

            (Self::Flame, ArmorType::Infantry) => Some(1.20),
            (Self::Flame, ArmorType::Light) => Some(0.35),
            (Self::Flame, ArmorType::Heavy) => Some(0.10),
            (Self::Flame, ArmorType::Heli) => Some(0.50),
            (Self::Flame, ArmorType::Plane) => None,
            (Self::Flame, ArmorType::Boat) => Some(0.15),
            (Self::Flame, ArmorType::Ship) => Some(0.05),
            (Self::Flame, ArmorType::Submarine) => Some(0.50),
            (Self::Flame, ArmorType::Structure) => Some(0.05),

            (Self::Rocket, ArmorType::Infantry) => Some(0.70),
            (Self::Rocket, ArmorType::Light) => Some(0.70),
            (Self::Rocket, ArmorType::Heavy) => Some(1.20),
            (Self::Rocket, ArmorType::Heli) => Some(1.10),
            (Self::Rocket, ArmorType::Plane) => None,
            (Self::Rocket, ArmorType::Boat) => Some(0.70),
            (Self::Rocket, ArmorType::Ship) => Some(1.20),
            (Self::Rocket, ArmorType::Submarine) => Some(1.00),
            (Self::Rocket, ArmorType::Structure) => Some(1.20),

            (Self::Torpedo, ArmorType::Boat) => Some(0.70),
            (Self::Torpedo, ArmorType::Ship) => Some(1.20),
            (Self::Torpedo, ArmorType::Submarine) => Some(1.00),
            (Self::Torpedo, ArmorType::Structure) => Some(1.00),
            (Self::Torpedo, _) => None,

            (Self::SurfaceMissiles, ArmorType::Infantry) => Some(1.20),
            (Self::SurfaceMissiles, ArmorType::Light) => Some(1.20),
            (Self::SurfaceMissiles, ArmorType::Heavy) => Some(1.00),
            (Self::SurfaceMissiles, ArmorType::Heli) => None,
            (Self::SurfaceMissiles, ArmorType::Plane) => None,
            (Self::SurfaceMissiles, ArmorType::Boat) => Some(1.20),
            (Self::SurfaceMissiles, ArmorType::Ship) => Some(1.00),
            (Self::SurfaceMissiles, ArmorType::Submarine) => Some(1.00),
            (Self::SurfaceMissiles, ArmorType::Structure) => Some(0.80),

            (Self::AirMissiles, ArmorType::Heli) => Some(1.20),
            (Self::AirMissiles, ArmorType::Plane) => Some(1.00),
            (Self::AirMissiles, _) => None,
        }
    }
    pub fn effect(&self, p: Point) -> Effect {
        match self {
            Self::Flame => Effect::Flame(p),
            Self::MachineGun => Effect::GunFire(p),
            Self::Shells => Effect::ShellFire(p),
            _ => Effect::ShellFire(p), // TODO
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArmorType {
	Infantry,
	Light,
	Heavy,
	Heli,
	Plane,
    Boat,
    Ship,
	Submarine,
	Structure,
}

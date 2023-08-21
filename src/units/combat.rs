use crate::{game::events::Effect, map::{point::Point, direction::Direction}};



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
    pub fn damage_factor(&self, armor: &ArmorType, in_water: bool) -> Option<f32> {
        if !in_water && *self == Self::Torpedo {
            return None;
        }
        match (self, armor) {
            (_, ArmorType::Unknown) => Some(0.1),
            (Self::MachineGun, ArmorType::Infantry) => Some(1.00),
            (Self::MachineGun, ArmorType::Light) => Some(0.15),
            (Self::MachineGun, ArmorType::Heavy) => Some(0.05),
            (Self::MachineGun, ArmorType::Heli) => Some(0.20),
            (Self::MachineGun, ArmorType::Plane) => None,
            (Self::MachineGun, ArmorType::Submarine) => if !in_water { Some(0.15) } else { None },
            (Self::MachineGun, ArmorType::Structure) => Some(0.20),

            (Self::Rifle, ArmorType::Infantry) => Some(1.20),
            (Self::Rifle, ArmorType::Light) => Some(0.25),
            (Self::Rifle, ArmorType::Heavy) => Some(0.15),
            (Self::Rifle, ArmorType::Heli) => Some(0.30),
            (Self::Rifle, ArmorType::Plane) => Some(0.15),
            (Self::Rifle, ArmorType::Submarine) => if !in_water { Some(0.15) } else { None },
            (Self::Rifle, ArmorType::Structure) => Some(0.10),

            (Self::Shells, ArmorType::Infantry) => Some(0.90),
            (Self::Shells, ArmorType::Light) => Some(1.10),
            (Self::Shells, ArmorType::Heavy) => Some(0.70),
            (Self::Shells, ArmorType::Heli) => None,
            (Self::Shells, ArmorType::Plane) => None,
            (Self::Shells, ArmorType::Submarine) => if !in_water { Some(1.10) } else { None },
            (Self::Shells, ArmorType::Structure) => Some(1.00),

            (Self::Bombs, ArmorType::Infantry) => Some(1.10),
            (Self::Bombs, ArmorType::Light) => Some(1.10),
            (Self::Bombs, ArmorType::Heavy) => Some(0.9),
            (Self::Bombs, ArmorType::Heli) => None,
            (Self::Bombs, ArmorType::Plane) => None,
            (Self::Bombs, ArmorType::Submarine) => if !in_water { Some(1.10) } else { None },
            (Self::Bombs, ArmorType::Structure) => Some(1.00),

            (Self::AntiAir, ArmorType::Heli) => Some(1.50),
            (Self::AntiAir, ArmorType::Plane) => Some(1.20),
            (Self::AntiAir, _) => None,

            (Self::Flame, ArmorType::Infantry) => Some(1.20),
            (Self::Flame, ArmorType::Light) => Some(0.35),
            (Self::Flame, ArmorType::Heavy) => Some(0.10),
            (Self::Flame, ArmorType::Heli) => Some(0.50),
            (Self::Flame, ArmorType::Plane) => None,
            (Self::Flame, ArmorType::Submarine) => if !in_water { Some(0.35) } else { None },
            (Self::Flame, ArmorType::Structure) => Some(0.05),

            (Self::Rocket, ArmorType::Infantry) => Some(0.70),
            (Self::Rocket, ArmorType::Light) => Some(0.70),
            (Self::Rocket, ArmorType::Heavy) => Some(1.20),
            (Self::Rocket, ArmorType::Heli) => Some(1.10),
            (Self::Rocket, ArmorType::Plane) => None,
            (Self::Rocket, ArmorType::Submarine) => if !in_water { Some(0.70) } else { None },
            (Self::Rocket, ArmorType::Structure) => Some(1.20),
            // in_water is checked at the top
            (Self::Torpedo, ArmorType::Infantry) => Some(0.90),
            (Self::Torpedo, ArmorType::Light) => Some(1.10),
            (Self::Torpedo, ArmorType::Heavy) => Some(0.70),
            (Self::Torpedo, ArmorType::Heli) => None,
            (Self::Torpedo, ArmorType::Plane) => None,
            (Self::Torpedo, ArmorType::Submarine) => Some(1.10),
            (Self::Torpedo, ArmorType::Structure) => Some(1.00),

            (Self::SurfaceMissiles, ArmorType::Infantry) => Some(1.20),
            (Self::SurfaceMissiles, ArmorType::Light) => Some(1.20),
            (Self::SurfaceMissiles, ArmorType::Heavy) => Some(1.00),
            (Self::SurfaceMissiles, ArmorType::Heli) => None,
            (Self::SurfaceMissiles, ArmorType::Plane) => None,
            (Self::SurfaceMissiles, ArmorType::Submarine) => if !in_water { Some(1.20) } else { None },
            (Self::SurfaceMissiles, ArmorType::Structure) => Some(0.80),

            (Self::AirMissiles, ArmorType::Heli) => Some(1.20),
            (Self::AirMissiles, ArmorType::Plane) => Some(1.00),
            (Self::AirMissiles, _) => None,
        }
    }
    pub fn effect<D: Direction>(&self, p: Point) -> Effect<D> {
        match self {
            Self::Flame => Effect::Flame(p),
            Self::MachineGun => Effect::GunFire(p),
            Self::Rifle => Effect::GunFire(p),
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
    Submarine,
	Structure,
    Unknown, // units half-hidden in light fog
}

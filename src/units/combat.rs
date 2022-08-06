

pub enum AttackType {
    None,
    Adjacent,
    Ranged(u8, u8),
    Straight(u8, u8),
}

pub enum WeaponType {
    MachineGun,
    Shells,
    AntiAir,
    Flame,
    Rocket,
    Torpedo,
    Rifle,
    // immobile ranged
    SurfaceMissiles,
    AirMissiles,
}
impl WeaponType {
    pub fn damage_factor(&self, armor: &ArmorType) -> Option<f32> {
        match (self, armor) {
            (Self::MachineGun, ArmorType::Infantry) => Some(1.00),
            (Self::MachineGun, ArmorType::Light) => Some(0.30),
            (Self::MachineGun, ArmorType::Heavy) => Some(0.10),
            (Self::MachineGun, ArmorType::Heli) => Some(0.30),
            (Self::MachineGun, ArmorType::Plane) => None,
            (Self::MachineGun, ArmorType::Submarine) => Some(0.40),
            (Self::MachineGun, ArmorType::Structure) => Some(0.20),

            (Self::Shells, ArmorType::Infantry) => Some(0.90),
            (Self::Shells, ArmorType::Light) => Some(1.10),
            (Self::Shells, ArmorType::Heavy) => Some(1.00),
            (Self::Shells, ArmorType::Heli) => None,
            (Self::Shells, ArmorType::Plane) => None,
            (Self::Shells, ArmorType::Submarine) => Some(1.00),
            (Self::Shells, ArmorType::Structure) => Some(1.00),
            
            (Self::AntiAir, ArmorType::Heli) => Some(1.50),
            (Self::AntiAir, ArmorType::Plane) => Some(1.20),
            (Self::AntiAir, _) => None,

            (Self::Flame, ArmorType::Infantry) => Some(1.20),
            (Self::Flame, ArmorType::Light) => Some(0.35),
            (Self::Flame, ArmorType::Heavy) => Some(0.10),
            (Self::Flame, ArmorType::Heli) => Some(0.50),
            (Self::Flame, ArmorType::Plane) => None,
            (Self::Flame, ArmorType::Submarine) => Some(0.50),
            (Self::Flame, ArmorType::Structure) => Some(0.05),

            (Self::Rocket, ArmorType::Infantry) => Some(0.70),
            (Self::Rocket, ArmorType::Light) => Some(0.70),
            (Self::Rocket, ArmorType::Heavy) => Some(1.20),
            (Self::Rocket, ArmorType::Heli) => Some(1.10),
            (Self::Rocket, ArmorType::Plane) => None,
            (Self::Rocket, ArmorType::Submarine) => Some(1.00),
            (Self::Rocket, ArmorType::Structure) => Some(1.20),

            (Self::Torpedo, ArmorType::Light) => Some(1.30),
            (Self::Torpedo, ArmorType::Heavy) => Some(0.70),
            (Self::Torpedo, ArmorType::Submarine) => Some(1.00),
            (Self::Torpedo, ArmorType::Structure) => Some(1.00),
            (Self::Torpedo, _) => None,

            (Self::Rifle, ArmorType::Infantry) => Some(1.10),
            (Self::Rifle, ArmorType::Light) => Some(0.75),
            (Self::Rifle, ArmorType::Heavy) => Some(0.20),
            (Self::Rifle, ArmorType::Heli) => Some(0.75),
            (Self::Rifle, ArmorType::Plane) => Some(0.20),
            (Self::Rifle, ArmorType::Submarine) => Some(0.10),
            (Self::Rifle, ArmorType::Structure) => Some(0.10),

            (Self::SurfaceMissiles, ArmorType::Infantry) => Some(1.20),
            (Self::SurfaceMissiles, ArmorType::Light) => Some(1.20),
            (Self::SurfaceMissiles, ArmorType::Heavy) => Some(1.00),
            (Self::SurfaceMissiles, ArmorType::Heli) => None,
            (Self::SurfaceMissiles, ArmorType::Plane) => None,
            (Self::SurfaceMissiles, ArmorType::Submarine) => Some(1.00),
            (Self::SurfaceMissiles, ArmorType::Structure) => Some(0.80),

            (Self::AirMissiles, ArmorType::Heli) => Some(1.20),
            (Self::AirMissiles, ArmorType::Plane) => Some(1.00),
            (Self::AirMissiles, _) => None,
        }
    }
}

pub enum ArmorType {
	Infantry,
	Light,
	Heavy,
	Heli,
	Plane,
	Submarine,
	Structure,
}

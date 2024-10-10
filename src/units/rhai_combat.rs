use rhai::*;
use rhai::plugin::*;

use crate::map::direction::*;
use crate::map::point::*;

macro_rules! combat_module {
    ($pack: ident, $name: ident, $d: ty) => {
        #[export_module]
        mod $name {
            pub type AttackVector = crate::units::combat::AttackVector<$d>;

            #[rhai_fn(pure, name = "==")]
            pub fn eq(p1: &mut AttackVector, p2: AttackVector) -> bool {
                *p1 == p2
            }
            #[rhai_fn(pure, name = "!=")]
            pub fn neq(u1: &mut AttackVector, u2: AttackVector) -> bool {
                *u1 != u2
            }

            #[rhai_fn(name = "AttackVector")]
            pub fn new_attack_vector(d: $d) -> AttackVector {
                AttackVector::Direction(d)
            }
            #[rhai_fn(name = "AttackVector")]
            pub fn new_attack_vector2(p: Point) -> AttackVector {
                AttackVector::Point(p)
            }
            #[rhai_fn(name = "AttackVector")]
            pub fn new_attack_vector3(p: Point, d: $d) -> AttackVector {
                AttackVector::DirectedPoint(p, d)
            }
        }

        def_package! {
            pub $pack(module)
            {
                combine_with_exported_module!(module, stringify!($name), $name);
            } |> |_engine| {
            }
        }
    };
}

combat_module!(CombatPackage4, combat_module4, Direction4);
combat_module!(CombatPackage6, combat_module6, Direction6);

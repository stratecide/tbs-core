use std::sync::Arc;

use crate::config::config::Config;
use crate::tags::tests::*;
use crate::terrain::TerrainType;
use crate::units::unit_types::UnitType;

use super::direction::*;
use super::map::Map;
use super::point::*;
use super::point_map::PointMap;
use super::wrapping_map::{Distortion, WMBuilder};

pub fn chess_board() -> Map<Direction4> {
    let config = Arc::new(Config::default());
    let map = PointMap::new(8, 8, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let environment = map.environment().clone();
    for p in map.all_points() {
        if p.y == 1 || p.y == 6 {
            map.set_terrain(p, TerrainType::ChessPawnTile.instance(&environment).build());
        } else {
            map.set_terrain(p, TerrainType::ChessTile.instance(&environment).build());
        }
    }
    // rooks
    map.set_unit(Point::new(0, 0), Some(UnitType::rook().instance(&environment).set_owner_id(1).build()));
    map.set_unit(Point::new(7, 0), Some(UnitType::rook().instance(&environment).set_owner_id(1).build()));
    map.set_unit(Point::new(0, 7), Some(UnitType::rook().instance(&environment).set_owner_id(0).build()));
    map.set_unit(Point::new(7, 7), Some(UnitType::rook().instance(&environment).set_owner_id(0).build()));
    // knights
    map.set_unit(Point::new(1, 0), Some(UnitType::knight().instance(&environment).set_owner_id(1).build()));
    map.set_unit(Point::new(6, 0), Some(UnitType::knight().instance(&environment).set_owner_id(1).build()));
    map.set_unit(Point::new(1, 7), Some(UnitType::knight().instance(&environment).set_owner_id(0).build()));
    map.set_unit(Point::new(6, 7), Some(UnitType::knight().instance(&environment).set_owner_id(0).build()));
    // bishops
    map.set_unit(Point::new(2, 0), Some(UnitType::bishop().instance(&environment).set_owner_id(1).build()));
    map.set_unit(Point::new(5, 0), Some(UnitType::bishop().instance(&environment).set_owner_id(1).build()));
    map.set_unit(Point::new(2, 7), Some(UnitType::bishop().instance(&environment).set_owner_id(0).build()));
    map.set_unit(Point::new(5, 7), Some(UnitType::bishop().instance(&environment).set_owner_id(0).build()));
    // queens and kings
    map.set_unit(Point::new(3, 0), Some(UnitType::queen().instance(&environment).set_owner_id(1).build()));
    map.set_unit(Point::new(4, 0), Some(UnitType::king().instance(&environment).set_owner_id(1).build()));
    map.set_unit(Point::new(3, 7), Some(UnitType::queen().instance(&environment).set_owner_id(0).build()));
    map.set_unit(Point::new(4, 7), Some(UnitType::king().instance(&environment).set_owner_id(0).build()));
    // pawns
    for x in 0..8 {
        map.set_unit(Point::new(x, 1), Some(UnitType::pawn().instance(&environment).set_tag(TAG_PAWN_DIRECTION, crate::tags::TagValue::Direction(Direction4::D270)).set_owner_id(1).build()));
        map.set_unit(Point::new(x, 6), Some(UnitType::pawn().instance(&environment).set_tag(TAG_PAWN_DIRECTION, crate::tags::TagValue::Direction(Direction4::D90)).set_owner_id(0).build()));
    }
    map
}

#[test_log::test]
fn simple_distortions() {
    let distortion = Distortion::neutral();
    for d in Direction4::list() {
        assert_eq!(d, distortion.update_direction(d));
        assert_eq!(d, distortion.update_diagonal_direction(d));
    }

    let distortion = Distortion::new(true, Direction6::D60);
    assert_eq!(distortion.update_direction(Direction6::D0), Direction6::D240);
    assert_eq!(distortion.update_direction(Direction6::D60), Direction6::D180);
    assert_eq!(distortion.update_direction(Direction6::D120), Direction6::D120);
    assert_eq!(distortion.update_direction(Direction6::D180), Direction6::D60);
    assert_eq!(distortion.update_direction(Direction6::D240), Direction6::D0);
    assert_eq!(distortion.update_direction(Direction6::D300), Direction6::D300);
    assert_eq!(distortion.update_diagonal_direction(Direction6::D0), Direction6::D180);
    assert_eq!(distortion.update_diagonal_direction(Direction6::D60), Direction6::D120);
    assert_eq!(distortion.update_diagonal_direction(Direction6::D120), Direction6::D60);
    assert_eq!(distortion.update_diagonal_direction(Direction6::D180), Direction6::D0);
    assert_eq!(distortion.update_diagonal_direction(Direction6::D240), Direction6::D300);
    assert_eq!(distortion.update_diagonal_direction(Direction6::D300), Direction6::D240);
}


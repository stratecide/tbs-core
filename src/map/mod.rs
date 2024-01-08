pub mod point;
pub mod point_map;
pub mod direction;
pub mod wrapping_map;
pub mod map;

#[cfg(test)]
mod tests {

    use interfaces::game_interface::*;
    use interfaces::map_interface::*;
    use crate::game::game::*;
    use crate::game::fog::*;
    use crate::map::direction::*;
    use crate::map::map::Map;
    use crate::map::wrapping_map::WrappingMapBuilder;
    
    #[test]
    fn export_import_map_d4() {
        // TODO
    }
}

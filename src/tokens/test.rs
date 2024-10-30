use std::sync::Arc;

use crate::config::config::Config;
use crate::config::environment::Environment;
use crate::game::commands::Command;
use crate::game::fog::{FogMode, FogSetting};
use crate::game::game::Game;
use crate::map::direction::*;
use crate::map::map::Map;
use crate::map::point::*;
use crate::map::point_map::{MapSize, PointMap};
use crate::map::wrapping_map::WMBuilder;
use crate::tags::tests::*;
use crate::tokens::token::Token;
use crate::units::commands::{UnitAction, UnitCommand};
use crate::units::movement::{Path, PathStep};
use crate::units::unit_types::UnitType;

use super::token_types::TokenType;

// helper functions

impl TokenType {
    pub const COINS: TokenType = TokenType(0);
    pub const BUBBLE: TokenType = TokenType(1);
    pub const SKULL: TokenType = TokenType(2);
    pub const SLUDGE: TokenType = TokenType(3);
}

// actual tests

#[test]
fn verify_token_test_constants() {
    let config = Arc::new(Config::test_config());
    let environment = Environment::new_map(config, MapSize::new(5, 5));
    assert_eq!(environment.config.token_name(TokenType::COINS), "CoinPile");
    assert_eq!(environment.config.token_name(TokenType::BUBBLE), "Bubble");
    assert_eq!(environment.config.token_name(TokenType::SKULL), "Skull");
    assert_eq!(environment.config.token_name(TokenType::SLUDGE), "Sludge");
}

#[test]
fn collect_coin_tokens() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(50).build_with_defaults()));
    map.set_unit(Point::new(4, 4), Some(UnitType::war_ship().instance(&map_env).set_owner_id(1).set_hp(100).build_with_defaults()));
    let coins = [1, 2].map(|factor| {
        let mut coins = Token::new(map_env.clone(), TokenType::COINS);
        coins.set_tag(TAG_COINS, factor.into());
        coins
    });
    map.set_tokens(Point::new(0, 0), vec![coins[0].clone()]);
    map.set_tokens(Point::new(2, 0), vec![coins[1].clone()]);
    let mut settings = map.settings().unwrap();
    for player in &mut settings.players {
        player.set_income(100);
    }
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    let (mut server, _) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    server.with(|game| {
        for player in &game.players {
            assert_eq!(*player.funds, 0);
        }
    });
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 0), vec![PathStep::Dir(Direction4::D0), PathStep::Dir(Direction4::D0)]),
        action: UnitAction::Wait,
    }), Arc::new(|| 0.)).unwrap();
    server.with(|game| {
        assert_eq!(game.get_map().get_tokens(Point::new(0, 0)), &[]);
        assert_eq!(game.get_map().get_tokens(Point::new(2, 0)), &[]);
        assert_eq!(*game.players[0].funds, 150);
        assert_eq!(*game.players[1].funds, 0);
    });
}

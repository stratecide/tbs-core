use std::sync::Arc;

use crate::config::config::Config;
use crate::config::environment::Environment;
use crate::game::commands::Command;
use crate::game::fog::{FogMode, FogSetting};
use crate::game::game::Game;
use crate::game::game_view::GameView;
use crate::map::direction::*;
use crate::map::map::Map;
use crate::map::point::*;
use crate::map::point_map::{MapSize, PointMap};
use crate::map::wrapping_map::WMBuilder;
use crate::script::custom_action::CustomActionInput;
use crate::tags::tests::*;
use crate::tokens::token::Token;
use crate::units::commands::{UnitAction, UnitCommand};
use crate::units::movement::{Path, PathStep};
use crate::units::unit_types::UnitType;

use super::token_types::TokenType;

// helper functions

impl TokenType {
    pub const COINS: TokenType = TokenType(0);
    pub const SKULL: TokenType = TokenType(1);
    pub const SLUDGE: TokenType = TokenType(2);
    pub const BUBBLE_AIRPORT: TokenType = TokenType(3);
    pub const BUBBLE_FACTORY: TokenType = TokenType(4);
    pub const BUBBLE_PORT: TokenType = TokenType(5);
}

// actual tests

#[test]
fn verify_token_test_constants() {
    let config = Arc::new(Config::test_config());
    let environment = Environment::new_map(config, MapSize::new(5, 5));
    assert_eq!(environment.config.token_name(TokenType::COINS), "CoinPile");
    assert_eq!(environment.config.token_name(TokenType::SKULL), "Skull");
    assert_eq!(environment.config.token_name(TokenType::SLUDGE), "Sludge");
    assert_eq!(environment.config.token_name(TokenType::BUBBLE_AIRPORT), "AirportBubble");
    assert_eq!(environment.config.token_name(TokenType::BUBBLE_FACTORY), "FactoryBubble");
    assert_eq!(environment.config.token_name(TokenType::BUBBLE_PORT), "PortBubble");
}

#[test]
fn collect_coin_tokens() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(5, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(50).build()));
    map.set_unit(Point::new(4, 4), Some(UnitType::war_ship().instance(&map_env).set_owner_id(1).set_hp(100).build()));
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

#[test]
fn bubble_token() {
    let config = Arc::new(Config::test_config());
    let map = PointMap::new(7, 5, false);
    let map = WMBuilder::<Direction4>::new(map);
    let mut map = Map::new(map.build(), &config);
    let map_env = map.environment().clone();
    map.set_unit(Point::new(0, 0), Some(UnitType::small_tank().instance(&map_env).set_owner_id(0).set_hp(50).build()));
    map.set_unit(Point::new(4, 4), Some(UnitType::war_ship().instance(&map_env).set_owner_id(1).set_hp(100).build()));
    let mut bubble = Token::new(map_env.clone(), TokenType::BUBBLE_FACTORY);
    bubble.set_owner_id(0);
    map.set_tokens(Point::new(1, 0), vec![bubble.clone()]);
    bubble.set_owner_id(2);
    map.set_tokens(Point::new(2, 0), vec![bubble]);
    let mut bubble = Token::new(map_env.clone(), TokenType::BUBBLE_AIRPORT);
    bubble.set_owner_id(0);
    map.set_tokens(Point::new(3, 0), vec![bubble]);
    let mut bubble = Token::new(map_env.clone(), TokenType::BUBBLE_PORT);
    bubble.set_owner_id(0);
    map.set_tokens(Point::new(4, 0), vec![bubble]);
    let mut settings = map.settings().unwrap();
    for player in &mut settings.players {
        player.set_funds(20000);
    }
    assert_eq!(settings.players.len(), 3);
    settings.fog_mode = FogMode::Constant(FogSetting::None);
    let (mut server, _) = Game::new_server(map.clone(), settings.build_default(), Arc::new(|| 0.));
    server.handle_command(Command::UnitCommand(UnitCommand {
        unload_index: None,
        path: Path::with_steps(Point::new(0, 0), vec![
            PathStep::Dir(Direction4::D0),
            PathStep::Dir(Direction4::D0),
            PathStep::Dir(Direction4::D0),
            PathStep::Dir(Direction4::D0),
            PathStep::Dir(Direction4::D0),
        ]),
        action: UnitAction::Wait,
    }), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(2, 0)), Vec::new());
    // factory bubble
    server.handle_command(Command::TokenAction(Point::new(1, 0), vec![
        CustomActionInput::ShopItem(UnitType::small_tank().0.into()),
    ].try_into().unwrap()), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(1, 0)), Vec::new());
    assert_eq!(server.get_unit(Point::new(1, 0)).unwrap(), UnitType::small_tank().instance(&server.environment()).set_owner_id(0).set_hp(100).build());
    // airport bubble
    server.handle_command(Command::TokenAction(Point::new(3, 0), vec![
        CustomActionInput::ShopItem(1.into()),
    ].try_into().unwrap()), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(3, 0)), Vec::new());
    assert_eq!(server.get_unit(Point::new(3, 0)).unwrap(), UnitType::attack_heli().instance(&server.environment()).set_owner_id(0).set_hp(100).build());
    // port bubble
    server.handle_command(Command::TokenAction(Point::new(4, 0), vec![
        CustomActionInput::ShopItem(7.into()),
    ].try_into().unwrap()), Arc::new(|| 0.)).unwrap();
    assert_eq!(server.get_tokens(Point::new(4, 0)), Vec::new());
    assert_eq!(server.get_unit(Point::new(4, 0)).unwrap(), UnitType::destroyer().instance(&server.environment()).set_owner_id(0).set_hp(100).build());
}

use interfaces::game_interface::*;
use interfaces::Perspective;
use semver::Version;
use uniform_smart_pointer::Urc;
use crate::commander::commander_type::CommanderType;
use crate::config::config::Config;
use crate::game::commands::Command;
use crate::game::game::*;
use crate::game::fog::*;
use crate::map::board::BoardView;
use crate::map::direction::*;
use crate::map::map::Map;
use crate::map::point::*;
use crate::map::point_map::PointMap;
use crate::map::wrapping_map::WMBuilder;
use crate::script::custom_action::CustomActionInput;
use crate::units::unit_types::UnitType;
use crate::VERSION;

#[test]
fn export_import_chess() {
    let version = Version::parse(VERSION).unwrap();
    let map = crate::map::test::chess_board();
    let config = map.environment().config.clone();
    let settings = map.settings().unwrap();

    for fog_setting in [FogSetting::None, FogSetting::Sharp(0)] {
        crate::debug!("fog setting: {fog_setting}");
        let mut settings = settings.clone();
        settings.fog_mode = FogMode::Constant(fog_setting);
        let perspective = Perspective::Team(0);
        let (server, events) = Game::new_server(map.clone(), &settings, settings.build_default(), Urc::new(|| 0.));
        let client = Game::new_client(map.clone(), &settings, settings.build_default(), events.get(&perspective).unwrap());
        let data = server.export();
        crate::debug!("data: {data:?}");
        let imported_server = Game::import_server(data.clone(), &config, version.clone()).unwrap();
        assert_eq!(server.get_fog(), imported_server.get_fog());
        assert_eq!(server.environment(), imported_server.environment());
        assert_eq!(server, imported_server);
        assert_eq!(client, Game::import_client(data.public.clone(), data.get_team(0), &config, version.clone()).unwrap());
    }
}

#[test]
fn changing_visibility() {
    let version = Version::parse(VERSION).unwrap();
    let config = Urc::new(Config::default());
    let map = PointMap::new(8, 8, false);
    let map = WMBuilder::<Direction6>::new(map);
    let mut map = Map::new(map.build(), &config);
    let environment = map.environment().clone();
    let origin = Point::new(0, 0);
    map.set_unit(origin, Some(UnitType::bazooka().instance(&environment).set_owner_id(0).build()));
    map.set_unit(Point::new(7, 7), Some(UnitType::small_tank().instance(&environment).set_owner_id(1).build()));
    let mut game_config = map.settings().unwrap();
    game_config.fog_mode = FogMode::Constant(FogSetting::Light(0));
    let mut settings = game_config.build_default();
    settings.players[0].set_commander(CommanderType::Tapio);
    let perspective = Perspective::Team(1);
    let (mut server, _) = Game::new_server(map.clone(), &game_config, settings, Urc::new(|| 0.));
    let commander = &mut server.players.get_mut(0).unwrap().commander;
    commander.add_charge(commander.get_max_charge() as i32);
    let exported = server.export();
    let mut client: Game<Direction6> = Game::import_client(exported.public, Some((1, exported.hidden.unwrap().teams.remove(&1).unwrap())), &config, version.clone()).unwrap();

    // unit presence should be visible in light fog
    assert_eq!(client.get_unit(origin).unwrap().typ(), UnitType::question_mark());

    // now create a forest at origin, which changes the unit's visibility
    let events = server.handle_command(Command::commander_power(1, vec![
        CustomActionInput::Point(origin),
    ]), Urc::new(|| 0.)).unwrap();
    for ev in events.get(&perspective).unwrap() {
        ev.apply(&mut client);
    }
    assert_eq!(client.get_unit(origin), None);
    // client should look the same whether by applying events or re-importing from the server
    let exported = server.export();
    let client2: Game<Direction6> = Game::import_client(exported.public, Some((1, exported.hidden.unwrap().teams.remove(&1).unwrap())), &config, version).unwrap();
    assert_eq!(client, client2);
}

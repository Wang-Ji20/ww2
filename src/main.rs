pub mod model;
pub mod tui;
pub mod systems;

use std::time::Duration;
use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;

use crate::model::game_state::GameState;
use crate::systems::{input_system, render_system, simulation_system, GameResource, SimClock, TuiResource};
use crate::tui::terminal::TuiManager;

fn main() {
    // 1. 初始化终端 TUI
    let tui_mgr = match TuiManager::init() {
        Ok(mgr) => mgr,
        Err(err) => {
            eprintln!("无法初始化终端界面: {}", err);
            return;
        }
    };

    // 2. 初始化 1939 经典二战推演初始状态
    let initial_scenario = GameState::new_1939_scenario();

    // 3. 构建基于最新 Bevy 0.20 的无头调度引擎
    App::new()
        .add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(30))))
        .insert_resource(GameResource(initial_scenario))
        .insert_resource(TuiResource(tui_mgr))
        .init_resource::<SimClock>()
        .add_systems(
            Update,
            (input_system, simulation_system, render_system).chain(),
        )
        .run();
}

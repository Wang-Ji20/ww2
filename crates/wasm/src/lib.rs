//! 浏览器外壳：将纯函数领域核心 (ww2-core) 通过 wasm-bindgen 暴露给前端。
//! 前端以 JSON 下发 `UserAction`，并在每帧读取整个 `GameState` 的 JSON 投影。

use serde::Serialize;
use wasm_bindgen::prelude::*;
use ww2_core::game_state::{GameState, UserAction};
use ww2_core::types::UnitStats;
use ww2_core::types::{ProvinceId, Terrain, UnitType};

/// 单帧最多推进的小时数，避免标签页休眠后一次性结算过多
const MAX_HOURS_PER_TICK: u32 = 24;

#[wasm_bindgen]
pub struct Game {
    // 以 Option 承载，便于按值传入纯函数管线而无需克隆整个状态
    state: Option<GameState>,
    accumulated_secs: f32,
}

#[wasm_bindgen]
impl Game {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Game {
        Game {
            state: Some(GameState::new_1939_scenario()),
            accumulated_secs: 0.0,
        }
    }

    /// 推进真实时间 `dt_secs` 秒，返回本次是否推进了游戏时间
    pub fn tick(&mut self, dt_secs: f32) -> bool {
        let state = self.state();
        if state.is_paused {
            return false;
        }

        // 速度 1: 真实 1 秒 = 1 小时; 速度 5: 真实 0.06 秒 = 1 小时
        let secs_per_hour = match state.speed {
            1 => 1.0,
            2 => 0.5,
            3 => 0.25,
            4 => 0.12,
            _ => 0.06,
        };

        self.accumulated_secs += dt_secs;
        let hours = (self.accumulated_secs / secs_per_hour).floor() as u32;
        if hours == 0 {
            return false;
        }
        self.accumulated_secs -= hours as f32 * secs_per_hour;

        let hours = hours.min(MAX_HOURS_PER_TICK);
        self.update(|s| s.step_pipeline(hours as f32));
        true
    }

    /// 重新开始 1939 开局
    pub fn reset(&mut self) {
        *self = Game::new();
    }

    /// 应用一个 JSON 编码的 `UserAction`，例如 `{"type":"SetSpeed","args":3}`
    pub fn act(&mut self, action_json: &str) -> Result<(), JsError> {
        Ok(self.apply_json(action_json)?)
    }

    /// 当前完整游戏状态的 JSON 投影
    #[wasm_bindgen(js_name = stateJson)]
    pub fn state_json(&self) -> String {
        serde_json::to_string(self.state()).expect("GameState 可序列化")
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    fn apply_json(&mut self, action_json: &str) -> Result<(), serde_json::Error> {
        let action: UserAction = serde_json::from_str(action_json)?;
        self.update(|s| s.handle_action(action));
        Ok(())
    }

    fn state(&self) -> &GameState {
        self.state.as_ref().expect("state 仅在 update 期间短暂取出")
    }

    fn update(&mut self, f: impl FnOnce(GameState) -> GameState) {
        self.state = Some(f(self.state.take().expect("state 仅在 update 期间短暂取出")));
    }
}

#[derive(Serialize)]
struct Catalog {
    provinces: Vec<ProvinceInfo>,
    terrains: Vec<TerrainInfo>,
    unit_types: Vec<UnitTypeInfo>,
}

#[derive(Serialize)]
struct ProvinceInfo {
    id: ProvinceId,
    name: &'static str,
}

#[derive(Serialize)]
struct TerrainInfo {
    id: Terrain,
    name: &'static str,
    attack_modifier: f32,
    defense_modifier: f32,
}

#[derive(Serialize)]
struct UnitTypeInfo {
    id: UnitType,
    name: &'static str,
    icon: &'static str,
    #[serde(flatten)]
    stats: UnitStats,
}

/// 静态显示数据（名称、地形修正、兵种属性），前端启动时读取一次
#[wasm_bindgen(js_name = catalogJson)]
pub fn catalog_json() -> String {
    const TERRAINS: [Terrain; 5] = [
        Terrain::Plains,
        Terrain::Forest,
        Terrain::Mountain,
        Terrain::Urban,
        Terrain::Fortified,
    ];
    const UNIT_TYPES: [UnitType; 3] = [UnitType::Infantry, UnitType::Panzer, UnitType::Motorized];

    let catalog = Catalog {
        provinces: ProvinceId::ALL
            .iter()
            .map(|&id| ProvinceInfo { id, name: id.name() })
            .collect(),
        terrains: TERRAINS
            .iter()
            .map(|&id| {
                let (attack_modifier, defense_modifier) = id.combat_modifiers();
                TerrainInfo { id, name: id.name(), attack_modifier, defense_modifier }
            })
            .collect(),
        unit_types: UNIT_TYPES
            .iter()
            .map(|&id| UnitTypeInfo { id, name: id.name(), icon: id.icon(), stats: id.base_stats() })
            .collect(),
    };
    serde_json::to_string(&catalog).expect("Catalog 可序列化")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play(game: &mut Game, json: &str) {
        game.apply_json(json).unwrap_or_else(|_| panic!("action should parse: {json}"));
    }

    #[test]
    fn actions_round_trip_through_json() {
        let mut game = Game::new();
        play(&mut game, r#"{"type":"TogglePause"}"#);
        play(&mut game, r#"{"type":"SetSpeed","args":5}"#);
        play(&mut game, r#"{"type":"SelectProvince","args":"Warsaw"}"#);
        play(&mut game, r#"{"type":"OrderAllUnitsInProvince","args":["Berlin","Danzig"]}"#);
        play(&mut game, r#"{"type":"QueueProduction","args":"Panzer"}"#);
        assert!(game.apply_json(r#"{"type":"Nope"}"#).is_err());

        let state: serde_json::Value = serde_json::from_str(&game.state_json()).unwrap();
        assert_eq!(state["speed"], 5);
        assert_eq!(state["is_paused"], false);
        assert_eq!(state["selected_province"], "Warsaw");
    }

    #[test]
    fn tick_advances_one_hour_at_a_time_and_caps() {
        let mut game = Game::new();
        assert!(!game.tick(10.0), "paused game must not advance");

        play(&mut game, r#"{"type":"TogglePause"}"#);
        play(&mut game, r#"{"type":"SetSpeed","args":1}"#);
        let start = game.state().date;
        assert!(game.tick(3.0));
        assert_eq!(game.state().date, start.advance_hours(3));

        // 长时间挂起后只追赶 MAX_HOURS_PER_TICK 小时
        assert!(game.tick(1000.0));
        assert_eq!(game.state().date, start.advance_hours(3 + MAX_HOURS_PER_TICK));
    }

    #[test]
    fn logs_are_capped() {
        let mut game = Game::new();
        // 军工厂分配满后，每次增产都会记录一条提示
        for _ in 0..(ww2_core::game_state::MAX_LOGS + 20) {
            play(&mut game, r#"{"type":"QueueProduction","args":"Infantry"}"#);
        }
        assert_eq!(game.state().logs.len(), ww2_core::game_state::MAX_LOGS);
    }

    #[test]
    fn catalog_lists_every_province() {
        let catalog: serde_json::Value = serde_json::from_str(&catalog_json()).unwrap();
        assert_eq!(catalog["provinces"].as_array().unwrap().len(), ProvinceId::ALL.len());
    }
}

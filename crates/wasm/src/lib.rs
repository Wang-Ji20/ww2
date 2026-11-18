//! 浏览器外壳：将纯函数领域核心 (ww2-core) 通过 wasm-bindgen 暴露给前端。
//! 前端读取模组文件后整体传入；之后以 JSON 下发 `UserAction`，
//! 启动时读取一次静态世界 (`worldJson`)，每帧读取动态状态 (`stateJson`)。

use wasm_bindgen::prelude::*;
use ww2_core::game_state::{GameState, UserAction};
use ww2_core::modpack::ModPack;
use ww2_core::types::CountryId;

/// 单帧最多推进的小时数，避免标签页休眠后一次性结算过多
const MAX_HOURS_PER_TICK: u32 = 24;

#[wasm_bindgen]
pub struct Game {
    // 以 Option 承载，便于按值传入纯函数管线而无需克隆整个状态
    state: Option<GameState>,
    /// 重新开局所需的初始状态（与当前对局共享静态世界）
    initial: GameState,
    accumulated_secs: f32,
}

#[wasm_bindgen]
impl Game {
    /// `layers_json`：按加载顺序排列的模组 `[{ id, files: [...] }]`；
    /// `player`：玩家国家标签，为空时使用剧本默认。数据有误时抛出包含全部错误的异常。
    #[wasm_bindgen(constructor)]
    pub fn new(layers_json: &str, scenario: &str, player: Option<String>) -> Result<Game, JsError> {
        Self::load(layers_json, scenario, player.as_deref()).map_err(|errors| JsError::new(&errors.join("\n")))
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

    /// 重新开始当前剧本
    pub fn reset(&mut self) {
        self.state = Some(self.initial.clone());
        self.accumulated_secs = 0.0;
    }

    /// 应用一个 JSON 编码的 `UserAction`，例如 `{"type":"SetSpeed","args":3}`
    pub fn act(&mut self, action_json: &str) -> Result<(), JsError> {
        Ok(self.apply_json(action_json)?)
    }

    /// 当前动态状态的 JSON 投影（不含静态世界）
    #[wasm_bindgen(js_name = stateJson)]
    pub fn state_json(&self) -> String {
        serde_json::to_string(self.state()).expect("GameState 可序列化")
    }

    /// 静态世界（地图、国家、兵种、地形、剧本信息）的 JSON，整局不变，启动时读取一次
    #[wasm_bindgen(js_name = worldJson)]
    pub fn world_json(&self) -> String {
        serde_json::to_string(&*self.state().world).expect("World 可序列化")
    }
}

impl Game {
    fn load(layers_json: &str, scenario: &str, player: Option<&str>) -> Result<Game, Vec<String>> {
        let pack = ModPack::from_layers_json(layers_json)?;
        let player = match player.filter(|p| !p.is_empty()) {
            Some(tag) => Some(CountryId::new(tag).ok_or_else(|| vec![format!("非法国家标签 {tag:?}")])?),
            None => None,
        };
        let initial = GameState::new(&pack, scenario, player)?;
        Ok(Game { state: Some(initial.clone()), initial, accumulated_secs: 0.0 })
    }

    /// 两局共享同一个静态世界
    #[cfg(test)]
    fn shares_world(&self) -> bool {
        std::sync::Arc::ptr_eq(&self.state().world, &self.initial.world)
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    const MOD_FILES: [(&str, &str); 4] = [
        ("base", include_str!("../../../web/public/mods/base/rules.json")),
        ("europe-mini", include_str!("../../../web/public/mods/europe-mini/countries.json")),
        ("europe-mini", include_str!("../../../web/public/mods/europe-mini/provinces.json")),
        ("europe-mini", include_str!("../../../web/public/mods/europe-mini/scenarios/1939.json")),
    ];

    fn layers_json() -> String {
        let file = |i: usize| serde_json::from_str::<serde_json::Value>(MOD_FILES[i].1).unwrap();
        serde_json::json!([
            { "id": MOD_FILES[0].0, "files": [file(0)] },
            { "id": MOD_FILES[1].0, "files": [file(1), file(2), file(3)] },
        ])
        .to_string()
    }

    fn new_game() -> Game {
        Game::load(&layers_json(), "1939", None).unwrap()
    }

    fn play(game: &mut Game, json: &str) {
        game.apply_json(json).unwrap_or_else(|_| panic!("action should parse: {json}"));
    }

    #[test]
    fn actions_round_trip_through_json() {
        let mut game = new_game();
        play(&mut game, r#"{"type":"TogglePause"}"#);
        play(&mut game, r#"{"type":"SetSpeed","args":5}"#);
        play(&mut game, r#"{"type":"SelectProvince","args":6}"#);
        play(&mut game, r#"{"type":"OrderAllUnitsInProvince","args":[1,5]}"#);
        play(&mut game, r#"{"type":"QueueProduction","args":"Panzer"}"#);
        play(&mut game, r#"{"type":"DeclareWar","args":"FRA"}"#);
        assert!(game.apply_json(r#"{"type":"DeclareWar","args":"france"}"#).is_err());
        assert!(game.apply_json(r#"{"type":"Nope"}"#).is_err());

        let state: serde_json::Value = serde_json::from_str(&game.state_json()).unwrap();
        assert_eq!(state["speed"], 5);
        assert_eq!(state["is_paused"], false);
        assert_eq!(state["selected_province"], 6);
        assert_eq!(state["controllers"]["5"], "POL");
        assert!(state.get("world").is_none(), "静态世界不随每帧状态下发");
    }

    #[test]
    fn tick_advances_one_hour_at_a_time_and_caps() {
        let mut game = new_game();
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
        let mut game = new_game();
        // 军工厂分配满后，每次增产都会记录一条提示
        for _ in 0..(ww2_core::game_state::MAX_LOGS + 20) {
            play(&mut game, r#"{"type":"QueueProduction","args":"Infantry"}"#);
        }
        assert_eq!(game.state().logs.len(), ww2_core::game_state::MAX_LOGS);
    }

    #[test]
    fn world_json_lists_every_province_and_country() {
        let game = new_game();
        let world: serde_json::Value = serde_json::from_str(&game.world_json()).unwrap();
        assert_eq!(world["map"]["provinces"].as_object().unwrap().len(), 17);
        assert_eq!(world["countries"]["GER"]["color"], "#8d939c");
        assert_eq!(world["capitals"]["POL"], 6);
        assert_eq!(world["unit_types"][0]["id"], "Infantry");
    }

    #[test]
    fn reset_restores_the_opening_and_keeps_the_world() {
        let mut game = new_game();
        play(&mut game, r#"{"type":"StepSingleHour"}"#);
        game.reset();
        assert_eq!(game.state().date, game.initial.date);
        assert!(game.shares_world());
    }

    #[test]
    fn broken_packs_and_unknown_players_are_rejected() {
        assert!(Game::load("[]", "1939", None).is_err());
        assert!(Game::load(&layers_json(), "1939", Some("ITA")).is_err());
        assert!(Game::load(&layers_json(), "1939", Some("nope")).is_err());
        let game = Game::load(&layers_json(), "1939", Some("SOV")).unwrap();
        assert_eq!(game.state().player_country.as_str(), "SOV");
    }
}

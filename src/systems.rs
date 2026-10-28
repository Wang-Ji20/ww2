use bevy::app::AppExit;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

use crate::model::game_state::GameState;
use crate::tui::terminal::{poll_input_event, InputOutcome, TuiManager};
use crate::tui::ui::render_hoi4_tui;

#[derive(Resource)]
pub struct GameResource(pub GameState);

#[derive(Resource)]
pub struct TuiResource(pub TuiManager);

#[derive(Resource)]
pub struct SimClock {
    pub accumulated_secs: f32,
}

impl Default for SimClock {
    fn default() -> Self {
        Self {
            accumulated_secs: 0.0,
        }
    }
}

/// 1. 输入处理系统：将终端原始事件纯函数式映射并应用到游戏状态中
pub fn input_system(
    mut game_res: ResMut<GameResource>,
    mut app_exit: MessageWriter<AppExit>,
) {
    let mode = game_res.0.mode;
    if let Ok(Some(outcome)) = poll_input_event(mode) {
        match outcome {
            InputOutcome::Quit => {
                app_exit.write(AppExit::Success);
            }
            InputOutcome::Action(action) => {
                let current_state = game_res.0.clone();
                game_res.0 = current_state.handle_action(action);
            }
        }
    }
}

/// 2. 推演仿真系统：以纯函数管道推进时间、交战、移驻与经济
pub fn simulation_system(
    time: Res<Time>,
    mut clock: ResMut<SimClock>,
    mut game_res: ResMut<GameResource>,
) {
    if game_res.0.is_paused {
        return;
    }

    let speed = game_res.0.speed; // 1 ~ 5
    // 速度 1: 真实 1 秒 = 1 小时; 速度 5: 真实 0.1 秒 = 1 小时
    let secs_per_hour = match speed {
        1 => 1.0,
        2 => 0.5,
        3 => 0.25,
        4 => 0.12,
        _ => 0.06,
    };

    clock.accumulated_secs += time.delta_secs();

    if clock.accumulated_secs >= secs_per_hour {
        let hours_to_advance = (clock.accumulated_secs / secs_per_hour).floor();
        clock.accumulated_secs -= hours_to_advance * secs_per_hour;

        let current_state = game_res.0.clone();
        game_res.0 = current_state.step_pipeline(hours_to_advance);
    }
}

/// 3. 渲染系统：纯声明式将不可变状态投影渲染至终端
pub fn render_system(
    game_res: Res<GameResource>,
    mut tui_res: ResMut<TuiResource>,
) {
    let _ = tui_res.0.terminal.draw(|frame| {
        render_hoi4_tui(frame, &game_res.0);
    });
}

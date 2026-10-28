use std::io::{self, stdout, Stdout};
use std::time::Duration;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::model::game_state::{InteractionMode, UserAction};
use crate::model::types::UnitType;

pub struct TuiManager {
    pub terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TuiManager {
    pub fn init() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    pub fn restore(&mut self) -> io::Result<()> {
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

impl Drop for TuiManager {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// 纯函数式键盘输入事件解析：输入 KeyEvent 与当前模式，返回映射后的 UserAction 或 Quit 标志
pub fn parse_input_event(key: KeyEvent, mode: InteractionMode) -> Option<InputOutcome> {
    match key.code {
        // 全局退出
        KeyCode::Char('q') | KeyCode::Char('Q') => Some(InputOutcome::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(InputOutcome::Quit),

        // 全局时间与速度控制
        KeyCode::Char(' ') => Some(InputOutcome::Action(UserAction::TogglePause)),
        KeyCode::Char('1') => Some(InputOutcome::Action(UserAction::SetSpeed(1))),
        KeyCode::Char('2') => Some(InputOutcome::Action(UserAction::SetSpeed(2))),
        KeyCode::Char('3') => Some(InputOutcome::Action(UserAction::SetSpeed(3))),
        KeyCode::Char('4') => Some(InputOutcome::Action(UserAction::SetSpeed(4))),
        KeyCode::Char('5') => Some(InputOutcome::Action(UserAction::SetSpeed(5))),
        KeyCode::Char('.') => Some(InputOutcome::Action(UserAction::StepSingleHour)),

        // 宣战快捷键 [W]
        KeyCode::Char('w') | KeyCode::Char('W') => Some(InputOutcome::Action(UserAction::DeclareWarNextEnemy)),

        // 生产快捷键 [P]
        KeyCode::Char('p') => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                Some(InputOutcome::Action(UserAction::QueueProduction(UnitType::Panzer)))
            } else {
                Some(InputOutcome::Action(UserAction::QueueProduction(UnitType::Infantry)))
            }
        }
        KeyCode::Char('P') => Some(InputOutcome::Action(UserAction::QueueProduction(UnitType::Panzer))),

        // 针对模式的按键
        _ => match mode {
            InteractionMode::Normal => parse_normal_mode_key(key),
            InteractionMode::Targeting { .. } => parse_targeting_mode_key(key),
        },
    }
}

fn parse_normal_mode_key(key: KeyEvent) -> Option<InputOutcome> {
    match key.code {
        // 地图导航
        KeyCode::Up | KeyCode::Char('k') => Some(InputOutcome::Action(UserAction::NavigateMap(0, -1))),
        KeyCode::Down | KeyCode::Char('j') => Some(InputOutcome::Action(UserAction::NavigateMap(0, 1))),
        KeyCode::Left | KeyCode::Char('h') => Some(InputOutcome::Action(UserAction::NavigateMap(-1, 0))),
        KeyCode::Right | KeyCode::Char('l') => Some(InputOutcome::Action(UserAction::NavigateMap(1, 0))),

        // 师团切换
        KeyCode::Tab => Some(InputOutcome::Action(UserAction::SelectNextDivision)),

        // 移驻 / 进攻部署
        KeyCode::Char('m') | KeyCode::Char('M') => {
            Some(InputOutcome::Action(UserAction::EnterTargetingMode { all_units: false }))
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            Some(InputOutcome::Action(UserAction::EnterTargetingMode { all_units: true }))
        }

        _ => None,
    }
}

fn parse_targeting_mode_key(key: KeyEvent) -> Option<InputOutcome> {
    match key.code {
        // 选择目标地块
        KeyCode::Up | KeyCode::Char('k') => Some(InputOutcome::Action(UserAction::NavigateMap(0, -1))),
        KeyCode::Down | KeyCode::Char('j') => Some(InputOutcome::Action(UserAction::NavigateMap(0, 1))),
        KeyCode::Left | KeyCode::Char('h') => Some(InputOutcome::Action(UserAction::NavigateMap(-1, 0))),
        KeyCode::Right | KeyCode::Char('l') => Some(InputOutcome::Action(UserAction::NavigateMap(1, 0))),

        // 确认目标
        KeyCode::Enter => Some(InputOutcome::Action(UserAction::ConfirmTargeting)),

        // 取消部署
        KeyCode::Esc => Some(InputOutcome::Action(UserAction::CancelTargetingMode)),

        _ => None,
    }
}

pub enum InputOutcome {
    Action(UserAction),
    Quit,
}

/// 非阻塞读取键盘输入
pub fn poll_input_event(mode: InteractionMode) -> io::Result<Option<InputOutcome>> {
    if event::poll(Duration::from_millis(15))? {
        if let Event::Key(key) = event::read()? {
            return Ok(parse_input_event(key, mode));
        }
    }
    Ok(None)
}

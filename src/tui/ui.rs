use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Gauge, List, ListItem, Paragraph, Wrap,
    },
    Frame,
};

use crate::model::division::{Division, DivisionOrder};
use crate::model::game_state::{GameState, InteractionMode};
use crate::model::types::{CountryId, ProvinceId};

pub fn render_hoi4_tui(frame: &mut Frame, state: &GameState) {
    let size = frame.area();

    // 垂直三层布局：顶部状态栏 (3), 主区 (Min), 底部生产与日志 (9)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(16),
            Constraint::Length(9),
        ])
        .split(size);

    render_top_dashboard(frame, state, main_chunks[0]);
    render_middle_war_room(frame, state, main_chunks[1]);
    render_bottom_deck(frame, state, main_chunks[2]);
}

/// 1. 顶部国家战略看板（Top Dashboard）
fn render_top_dashboard(frame: &mut Frame, state: &GameState, area: Rect) {
    let player = &state.countries[&state.player_country];

    let status_badge = if state.is_paused {
        Span::styled(" [ 暂停中 - 空格继续 ] ", Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(format!(" [ 推演中 {}x ] ", state.speed), Style::default().bg(Color::Green).fg(Color::Black).add_modifier(Modifier::BOLD))
    };

    let total_divisions = state.divisions.values()
        .filter(|d| d.country == state.player_country)
        .count();

    let top_text = vec![
        Line::from(vec![
            Span::styled("★ 钢铁雄心4·极简欧战推演 ★  ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format!("📅 {} ", state.date), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            status_badge,
            Span::raw(" | "),
            Span::styled("⚡ 世界紧张度: ", Style::default().fg(Color::LightRed)),
            Span::styled(format!("{:.1}%  ", state.world_tension), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw("| "),
            Span::styled("🚩 德意志国: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("PP: ", Style::default().fg(Color::Yellow)),
            Span::styled(format!("{:.0}  ", player.political_power), Style::default().fg(Color::White)),
            Span::styled("人力: ", Style::default().fg(Color::Green)),
            Span::styled(format!("{:.1}M  ", player.manpower / 1_000_000.0), Style::default().fg(Color::White)),
            Span::styled("民工: ", Style::default().fg(Color::Blue)),
            Span::styled(format!("{}  ", player.civ_factories), Style::default().fg(Color::White)),
            Span::styled("军工: ", Style::default().fg(Color::LightMagenta)),
            Span::styled(format!("{}  ", player.mil_factories), Style::default().fg(Color::White)),
            Span::styled("陆军: ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{}个师", total_divisions), Style::default().fg(Color::White)),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(top_text).block(block);
    frame.render_widget(paragraph, area);
}

/// 2. 中间战区主界面（左右分割）
fn render_middle_war_room(frame: &mut Frame, state: &GameState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(56), // 左侧：欧洲战略地图
            Constraint::Percentage(44), // 右侧：情报与部队指挥卡
        ])
        .split(area);

    render_war_map(frame, state, chunks[0]);
    render_intel_and_command(frame, state, chunks[1]);
}

/// 绘制欧洲战略战区地图（War Map）
fn render_war_map(frame: &mut Frame, state: &GameState, area: Rect) {
    let block = match state.mode {
        InteractionMode::Normal => Block::default()
            .title(" 🗺 欧洲战区战略态势图 (Europe 1939 Theater) ")
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(Color::Cyan)),
        InteractionMode::Targeting { all_units, .. } => Block::default()
            .title(if all_units {
                " 🎯【全军突击】请选择目标地块，按 [Enter] 发动总攻，[Esc] 取消 "
            } else {
                " 🎯【师团进军】请选择目标地块，按 [Enter] 确认移动，[Esc] 取消 "
            })
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .border_style(Style::default().fg(Color::Yellow)),
    };

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // 辅助闭包：获取省份的展示文本与颜色
    let get_prov_repr = |p_id: ProvinceId| -> (String, Color, bool, usize, Option<f32>) {
        let prov = &state.map.provinces[&p_id];
        let is_selected = state.selected_province == p_id;

        let controller_color = match prov.controller {
            CountryId::Germany => Color::LightYellow,
            CountryId::Poland => Color::Magenta,
            CountryId::France => Color::Blue,
            CountryId::UnitedKingdom => Color::Cyan,
            CountryId::SovietUnion => Color::Red,
        };

        // 驻军数
        let unit_count = state.divisions.values().filter(|d| d.location == p_id).count();

        // 战斗信息
        let combat_prog = state.combats.values()
            .find(|c| c.province == p_id)
            .map(|c| c.progress);

        let name = prov.name;
        (name.to_string(), controller_color, is_selected, unit_count, combat_prog)
    };

    // 拓扑节点行
    // Row 0: London
    // Row 1: Dover
    // Row 2: Calais, Cologne, Berlin, Danzig, EastPrussia, Leningrad
    // Row 3: Paris, Maginot, Munich, Warsaw, Minsk, Moscow
    // Row 4: Krakow, Kiev, Stalingrad

    let format_node = |id: ProvinceId| -> Span {
        let (name, color, selected, units, combat) = get_prov_repr(id);
        let flag = state.map.provinces[&id].controller.flag();

        let mut text = format!("{}{} [{}师]", flag, name, units);
        if let Some(prog) = combat {
            text = format!("⚔{:.0}% {}", prog, text);
        }

        if selected {
            Span::styled(
                format!("▶[{}]◀", text),
                Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                format!("[{}]", text),
                Style::default().fg(color).add_modifier(if units > 0 { Modifier::BOLD } else { Modifier::empty() }),
            )
        }
    };

    let map_lines = vec![
        Line::from(vec![
            Span::raw("   "),
            format_node(ProvinceId::London),
            Span::raw(" (大西洋/英吉利海峡)"),
        ]),
        Line::from(vec![
            Span::raw("       │"),
        ]),
        Line::from(vec![
            Span::raw("   "),
            format_node(ProvinceId::Dover),
            Span::raw(" ─── [渡口] ──┐"),
        ]),
        Line::from(vec![
            Span::raw("                     │"),
        ]),
        Line::from(vec![
            format_node(ProvinceId::Calais),
            Span::raw(" ── "),
            format_node(ProvinceId::Cologne),
            Span::raw(" ── "),
            format_node(ProvinceId::Berlin),
            Span::raw(" ── "),
            format_node(ProvinceId::Danzig),
            Span::raw(" ── "),
            format_node(ProvinceId::EastPrussia),
            Span::raw(" ── "),
            format_node(ProvinceId::Leningrad),
        ]),
        Line::from(vec![
            Span::raw("    │             │             │             │                │               │"),
        ]),
        Line::from(vec![
            format_node(ProvinceId::Paris),
            Span::raw(" ── "),
            format_node(ProvinceId::Maginot),
            Span::raw("   "),
            format_node(ProvinceId::Munich),
            Span::raw(" ── "),
            format_node(ProvinceId::Warsaw),
            Span::raw(" ─────── "),
            format_node(ProvinceId::Minsk),
            Span::raw(" ─────── "),
            format_node(ProvinceId::Moscow),
        ]),
        Line::from(vec![
            Span::raw("                                │             │                │               │"),
        ]),
        Line::from(vec![
            Span::raw("                          "),
            format_node(ProvinceId::Krakow),
            Span::raw(" ────────── "),
            format_node(ProvinceId::Kiev),
            Span::raw(" ─────────────── "),
            format_node(ProvinceId::Stalingrad),
        ]),
        Line::from(vec![
            Span::raw(""),
        ]),
        Line::from(vec![
            Span::styled("图例: ", Style::default().fg(Color::DarkGray)),
            Span::styled("卐德国(黄) ", Style::default().fg(Color::LightYellow)),
            Span::styled("🇵🇱波兰(洋红) ", Style::default().fg(Color::Magenta)),
            Span::styled("🇫🇷法国(蓝) ", Style::default().fg(Color::Blue)),
            Span::styled("🇬🇧英国(青) ", Style::default().fg(Color::Cyan)),
            Span::styled("☭苏联(红) ", Style::default().fg(Color::Red)),
            Span::styled("⚔交战地块(闪烁)", Style::default().fg(Color::LightRed)),
        ]),
        Line::from(vec![
            Span::styled("战略说明: 方向键/WASD切换选区，[M]移动进军，[A]驻军全线总突击！", Style::default().fg(Color::Gray)),
        ]),
    ];

    let p = Paragraph::new(map_lines).wrap(Wrap { trim: false });
    frame.render_widget(p, inner);
}

/// 3. 右侧：情报与部队指挥卡（Intel & Command Panel）
fn render_intel_and_command(frame: &mut Frame, state: &GameState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // 省份情报
            Constraint::Min(8),    // 师团详情卡片
        ])
        .split(area);

    render_province_intel(frame, state, chunks[0]);
    render_division_card(frame, state, chunks[1]);
}

fn render_province_intel(frame: &mut Frame, state: &GameState, area: Rect) {
    let prov = &state.map.provinces[&state.selected_province];
    let owner_c = &state.countries[&prov.original_owner];
    let ctrl_c = &state.countries[&prov.controller];

    let (att_mod, def_mod) = prov.terrain.combat_modifiers();

    let title = format!(" 📌 地块情报: {} ", prov.name);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightBlue));

    let is_occupied = prov.original_owner != prov.controller;
    let ctrl_status = if is_occupied {
        Span::styled(format!("{} (占领中!)", ctrl_c.id.name()), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(format!("{}", ctrl_c.id.name()), Style::default().fg(Color::Green))
    };

    let intel_lines = vec![
        Line::from(vec![
            Span::raw("控制方: "),
            ctrl_status,
            Span::raw(" | 法理归属: "),
            Span::styled(format!("{}", owner_c.id.short_name()), Style::default().fg(Color::White)),
            Span::raw(" | 胜利点: ★"),
            Span::styled(format!("{}", prov.victory_points), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw("地形: "),
            Span::styled(prov.terrain.name(), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
            Span::raw(format!(" (攻方修正: {:.0}%, 守方修正: {:.0}%)", att_mod * 100.0, def_mod * 100.0)),
        ]),
        Line::from(vec![
            Span::raw("工业设施: 民工 "),
            Span::styled(format!("{}", prov.civ_factories), Style::default().fg(Color::Blue)),
            Span::raw(" 所 | 军工 "),
            Span::styled(format!("{}", prov.mil_factories), Style::default().fg(Color::Magenta)),
            Span::raw(" 所 | 补给基建: "),
            Span::styled(format!("{}/10", prov.infrastructure), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("邻接走廊: "),
            Span::styled(
                prov.neighbors.iter().map(|n| state.map.provinces[n].name).collect::<Vec<_>>().join(" · "),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];

    let p = Paragraph::new(intel_lines).block(block);
    frame.render_widget(p, area);
}

fn render_division_card(frame: &mut Frame, state: &GameState, area: Rect) {
    let block = Block::default()
        .title(" ⚔ 驻防编制与师团卡片 (按[Tab]切换) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // 获取当前省份的所有师团
    let prov_units: Vec<&Division> = state.divisions.values()
        .filter(|d| d.location == state.selected_province)
        .collect();

    if prov_units.is_empty() {
        let empty_p = Paragraph::new("该省份当前无任何武装力量驻守。").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty_p, inner);
        return;
    }

    // 选中的师团
    let active_div = state.selected_division
        .and_then(|id| state.divisions.get(&id))
        .filter(|d| d.location == state.selected_province)
        .or_else(|| prov_units.first().copied());

    let card_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // 驻军精简列表
            Constraint::Min(6),    // 选中师团具体属性
        ])
        .split(inner);

    // 1. 省内驻扎师团列表
    let unit_items: Vec<ListItem> = prov_units.iter().map(|d| {
        let is_selected = active_div.map_or(false, |act| act.id == d.id);
        let prefix = if is_selected { "▶ " } else { "  " };
        let flag = d.country.flag();
        let org_color = if d.organization > 30.0 { Color::Green } else { Color::Red };

        let status_desc = match &d.order {
            DivisionOrder::Idle => "待命",
            DivisionOrder::Moving { .. } => "行军中",
            DivisionOrder::Fighting { .. } => "⚔交战中",
        };

        let line = Line::from(vec![
            Span::raw(prefix),
            Span::raw(format!("{} {} ", flag, d.unit_type.icon())),
            Span::styled(format!("{:<16}", d.name), if is_selected { Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow) } else { Style::default().fg(Color::White) }),
            Span::styled(format!("组织:{:.0}% ", (d.organization / d.max_organization) * 100.0), Style::default().fg(org_color)),
            Span::styled(format!("兵力:{:.0}% ", d.strength), Style::default().fg(Color::LightGreen)),
            Span::styled(format!("[{}]", status_desc), Style::default().fg(Color::Cyan)),
        ]);
        ListItem::new(line)
    }).collect();

    let list = List::new(unit_items);
    frame.render_widget(list, card_chunks[0]);

    // 2. 选中的具体师团卡片
    if let Some(div) = active_div {
        let (eff_soft, eff_hard, eff_def, eff_brk) = div.effective_combat_stats();

        let org_ratio = (div.organization / div.max_organization).clamp(0.0, 1.0);
        let hp_ratio = (div.strength / 100.0).clamp(0.0, 1.0);

        let div_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // 组织度 Gauge
                Constraint::Length(1), // 兵力 Gauge
                Constraint::Min(4),    // 详细数值
            ])
            .split(card_chunks[1]);

        let org_gauge = Gauge::default()
            .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
            .ratio(org_ratio as f64)
            .label(format!("组织度 (ORG): {:.0}/{:.0}", div.organization, div.max_organization));
        frame.render_widget(org_gauge, div_layout[0]);

        let hp_gauge = Gauge::default()
            .gauge_style(Style::default().fg(Color::LightGreen).bg(Color::DarkGray))
            .ratio(hp_ratio as f64)
            .label(format!("部队耐久 (HP): {:.0}%", div.strength));
        frame.render_widget(hp_gauge, div_layout[1]);

        let stats_lines = vec![
            Line::from(vec![
                Span::raw("软攻(人员杀伤): "),
                Span::styled(format!("{:.1}  ", eff_soft), Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
                Span::raw("硬攻(反装甲): "),
                Span::styled(format!("{:.1}  ", eff_hard), Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)),
                Span::raw("防御力: "),
                Span::styled(format!("{:.1}  ", eff_def), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
                Span::raw("突破力: "),
                Span::styled(format!("{:.1}", eff_brk), Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("移速: "),
                Span::styled(format!("{:.0} km/h  ", div.speed_kmh), Style::default().fg(Color::White)),
                Span::raw("编制类型: "),
                Span::styled(div.unit_type.name(), Style::default().fg(Color::Yellow)),
                Span::raw(" | 所属统帅部: "),
                Span::styled(div.country.name(), Style::default().fg(Color::White)),
            ]),
        ];
        let stats_p = Paragraph::new(stats_lines);
        frame.render_widget(stats_p, div_layout[2]);
    }
}

/// 4. 底部生产线、事件日志与操作提示（Bottom Deck）
fn render_bottom_deck(frame: &mut Frame, state: &GameState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(32), // 军工生产队列
            Constraint::Percentage(68), // 战况日志与快捷键提示
        ])
        .split(area);

    render_production_queue(frame, state, chunks[0]);
    render_logs_and_help(frame, state, chunks[1]);
}

fn render_production_queue(frame: &mut Frame, state: &GameState, area: Rect) {
    let player = &state.countries[&state.player_country];

    let block = Block::default()
        .title(" 🏭 军工生产线 [按P增产步兵/Shift+P装甲] ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::LightMagenta));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if player.production_queue.is_empty() {
        let p = Paragraph::new("生产线空闲中，按 [P] 投入军工厂生产步兵师。").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    }

    let items: Vec<ListItem> = player.production_queue.iter().map(|line| {
        let (_, _, _, _, _, _, cost, _) = line.unit_type.base_stats();
        let pct = (line.progress / cost * 100.0).clamp(0.0, 100.0);
        let line_text = Line::from(vec![
            Span::styled(format!("{} ", line.unit_type.icon()), Style::default().fg(Color::Yellow)),
            Span::raw(format!("{:<8} ({}工厂) ", line.unit_type.name(), line.assigned_factories)),
            Span::styled(format!("[{:.0}%]", pct), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]);
        ListItem::new(line_text)
    }).collect();

    let list = List::new(items);
    frame.render_widget(list, inner);
}

fn render_logs_and_help(frame: &mut Frame, state: &GameState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),    // 战报滚动
            Constraint::Length(1), // 快捷键导航
        ])
        .split(area);

    let block = Block::default()
        .title(" 📜 战报简讯 (War Ticker) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Gray));

    // 取最后 3 条日志
    let log_lines: Vec<Line> = state.logs.iter().rev().take(3).rev().map(|log| {
        let color = if log.contains("大捷") || log.contains("宣战") {
            Color::Yellow
        } else if log.contains("溃退") || log.contains("包围") || log.contains("投降") {
            Color::LightRed
        } else if log.contains("战斗") {
            Color::LightCyan
        } else {
            Color::White
        };
        Line::from(Span::styled(log, Style::default().fg(color)))
    }).collect();

    let log_p = Paragraph::new(log_lines).block(block);
    frame.render_widget(log_p, chunks[0]);

    // 快捷键提示条
    let help_spans = vec![
        Span::styled("[空格] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("暂停/推演  "),
        Span::styled("[1-5] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("倍速  "),
        Span::styled("[.] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("单步推进  "),
        Span::styled("[WASD/方向键] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("切换地块  "),
        Span::styled("[Tab] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("切换师团  "),
        Span::styled("[M] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("移驻/进攻  "),
        Span::styled("[A] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("全军突击  "),
        Span::styled("[P] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("招募师团  "),
        Span::styled("[Q] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw("退出游戏"),
    ];
    let help_p = Paragraph::new(Line::from(help_spans)).alignment(Alignment::Center);
    frame.render_widget(help_p, chunks[1]);
}

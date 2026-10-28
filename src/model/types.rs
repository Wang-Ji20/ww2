use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CountryId {
    Germany,
    Poland,
    France,
    UnitedKingdom,
    SovietUnion,
}

impl CountryId {
    pub const ALL: [CountryId; 5] = [
        CountryId::Germany,
        CountryId::Poland,
        CountryId::France,
        CountryId::UnitedKingdom,
        CountryId::SovietUnion,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            CountryId::Germany => "德意志国 (Germany)",
            CountryId::Poland => "波兰共和国 (Poland)",
            CountryId::France => "法兰西共和国 (France)",
            CountryId::UnitedKingdom => "大不列颠联合王国 (UK)",
            CountryId::SovietUnion => "苏维埃社会主义共和国联盟 (USSR)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            CountryId::Germany => "GER",
            CountryId::Poland => "POL",
            CountryId::France => "FRA",
            CountryId::UnitedKingdom => "ENG",
            CountryId::SovietUnion => "SOV",
        }
    }

    pub fn flag(&self) -> &'static str {
        match self {
            CountryId::Germany => "卐",
            CountryId::Poland => "🇵🇱",
            CountryId::France => "🇫🇷",
            CountryId::UnitedKingdom => "🇬🇧",
            CountryId::SovietUnion => "☭",
        }
    }
}

impl fmt::Display for CountryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProvinceId {
    Berlin = 0,
    Cologne = 1,
    Munich = 2,
    EastPrussia = 3,
    Danzig = 4,
    Warsaw = 5,
    Krakow = 6,
    Paris = 7,
    Maginot = 8,
    Calais = 9,
    London = 10,
    Dover = 11,
    Minsk = 12,
    Kiev = 13,
    Leningrad = 14,
    Moscow = 15,
    Stalingrad = 16,
}

impl ProvinceId {
    pub const ALL: [ProvinceId; 17] = [
        ProvinceId::Berlin,
        ProvinceId::Cologne,
        ProvinceId::Munich,
        ProvinceId::EastPrussia,
        ProvinceId::Danzig,
        ProvinceId::Warsaw,
        ProvinceId::Krakow,
        ProvinceId::Paris,
        ProvinceId::Maginot,
        ProvinceId::Calais,
        ProvinceId::London,
        ProvinceId::Dover,
        ProvinceId::Minsk,
        ProvinceId::Kiev,
        ProvinceId::Leningrad,
        ProvinceId::Moscow,
        ProvinceId::Stalingrad,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            ProvinceId::Berlin => "柏林 (Berlin)",
            ProvinceId::Cologne => "科隆 (Cologne)",
            ProvinceId::Munich => "慕尼黑 (Munich)",
            ProvinceId::EastPrussia => "东普鲁士 (Königsberg)",
            ProvinceId::Danzig => "但泽走廊 (Danzig)",
            ProvinceId::Warsaw => "华沙 (Warsaw)",
            ProvinceId::Krakow => "克拉科夫 (Krakow)",
            ProvinceId::Paris => "巴黎 (Paris)",
            ProvinceId::Maginot => "马奇诺防线 (Maginot)",
            ProvinceId::Calais => "加来海峡 (Calais)",
            ProvinceId::London => "伦敦 (London)",
            ProvinceId::Dover => "多佛尔 (Dover)",
            ProvinceId::Minsk => "明斯克 (Minsk)",
            ProvinceId::Kiev => "基辅 (Kiev)",
            ProvinceId::Leningrad => "列宁格勒 (Leningrad)",
            ProvinceId::Moscow => "莫斯科 (Moscow)",
            ProvinceId::Stalingrad => "斯大林格勒 (Stalingrad)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terrain {
    Plains,
    Forest,
    Mountain,
    Urban,
    Fortified,
}

impl Terrain {
    pub fn name(&self) -> &'static str {
        match self {
            Terrain::Plains => "平原",
            Terrain::Forest => "森林",
            Terrain::Mountain => "山地",
            Terrain::Urban => "城市",
            Terrain::Fortified => "要塞",
        }
    }

    /// 函数式计算地形修正（攻方修正、守方修正）
    pub fn combat_modifiers(&self) -> (f32, f32) {
        match self {
            Terrain::Plains => (1.0, 1.0),      // 平原攻守均衡，装甲突破有优势
            Terrain::Forest => (0.8, 1.15),     // 森林攻方劣势
            Terrain::Mountain => (0.6, 1.4),    // 山地极难攻打
            Terrain::Urban => (0.7, 1.3),       // 城市巷战守方优势
            Terrain::Fortified => (0.5, 1.6),   // 坚固永备工事守方大幅优势
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitType {
    Infantry,
    Panzer,
    Motorized,
}

impl UnitType {
    pub fn name(&self) -> &'static str {
        match self {
            UnitType::Infantry => "步兵师",
            UnitType::Panzer => "装甲师",
            UnitType::Motorized => "机械化师",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            UnitType::Infantry => "[Inf]",
            UnitType::Panzer => "[Arm]",
            UnitType::Motorized => "[Mot]",
        }
    }

    /// 基础属性：(最大组织度, 软攻, 硬攻, 防御, 突破, 速度km/h, 军工需求, 人力需求)
    pub fn base_stats(&self) -> (f32, f32, f32, f32, f32, f32, f32, f32) {
        match self {
            UnitType::Infantry => (60.0, 24.0, 4.0, 32.0, 8.0, 4.0, 50.0, 10_000.0),
            UnitType::Panzer => (40.0, 36.0, 28.0, 18.0, 38.0, 12.0, 150.0, 8_000.0),
            UnitType::Motorized => (50.0, 28.0, 10.0, 24.0, 20.0, 9.0, 90.0, 9_000.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
}

impl GameDate {
    pub fn start_date() -> Self {
        Self {
            year: 1939,
            month: 9,
            day: 1,
            hour: 6,
        }
    }

    /// 纯函数推进时间
    pub fn advance_hours(&self, hours: u32) -> Self {
        let total_hours = self.hour as u32 + hours;
        let add_days = total_hours / 24;
        let new_hour = (total_hours % 24) as u8;

        let mut y = self.year;
        let mut m = self.month;
        let mut d = self.day as u32 + add_days;

        loop {
            let days_in_cur_month = match m {
                2 => 28,
                4 | 6 | 9 | 11 => 30,
                _ => 31,
            };
            if d <= days_in_cur_month {
                break;
            }
            d -= days_in_cur_month;
            m += 1;
            if m > 12 {
                m = 1;
                y += 1;
            }
        }

        Self {
            year: y,
            month: m,
            day: d as u8,
            hour: new_hour,
        }
    }
}

impl fmt::Display for GameDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}年{:02}月{:02}日 {:02}:00",
            self.year, self.month, self.day, self.hour
        )
    }
}

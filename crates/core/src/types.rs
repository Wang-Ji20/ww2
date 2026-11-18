use std::fmt;

/// 省份编号，由模组数据定义
pub type ProvinceId = u32;

/// 国家标签（如 "GER"）：1–3 位大写字母或数字，按值复制
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CountryId([u8; 3]);

impl CountryId {
    /// 解析国家标签，不合法时返回 None
    pub fn new(tag: &str) -> Option<Self> {
        let bytes = tag.as_bytes();
        let valid = (1..=3).contains(&bytes.len())
            && bytes.iter().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
        valid.then(|| {
            let mut buf = [0u8; 3];
            buf[..bytes.len()].copy_from_slice(bytes);
            Self(buf)
        })
    }

    pub fn as_str(&self) -> &str {
        let len = self.0.iter().position(|&b| b == 0).unwrap_or(3);
        std::str::from_utf8(&self.0[..len]).expect("标签只含 ASCII")
    }
}

impl fmt::Display for CountryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for CountryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

impl serde::Serialize for CountryId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for CountryId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let tag = String::deserialize(d)?;
        CountryId::new(&tag)
            .ok_or_else(|| serde::de::Error::custom(format!("非法国家标签 {tag:?}（应为 1–3 位大写字母或数字）")))
    }
}

/// 兵种编号（如 "Infantry"），由模组数据定义
pub type UnitTypeId = String;

/// 地形编号（如 "Plains"），由模组数据定义
pub type TerrainId = String;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UnitStats {
    pub max_organization: f32,
    pub soft_attack: f32,
    pub hard_attack: f32,
    pub defense: f32,
    pub breakthrough: f32,
    pub speed_kmh: f32,
    /// 完成一个师所需的军工产出
    pub production_cost: f32,
    /// 师团完工部署时消耗的人力
    pub manpower_cost: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
            let days_in_cur_month = days_in_month(y, m);
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

fn is_leap_year(year: u16) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

fn days_in_month(year: u16, month: u8) -> u32 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

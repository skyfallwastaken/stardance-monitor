use std::collections::HashMap;

use crate::scraper::{Accessory, Region};
use strum::VariantArray;

pub const EMOJI_COOKIES: &str = ":cookie:";
pub const EMOJI_TROLLEY: &str = ":tw_shopping_trolley:";
pub const EMOJI_NEW: &str = ":new:";
pub const EMOJI_TRASH: &str = ":win10-trash:";
pub const EMOJI_STAR: &str = ":star:";
pub const EMOJI_ROBOT: &str = ":robot_face:";
pub const EMOJI_MEDAL: &str = ":tw_medal:";

pub fn prices_changed(old: &HashMap<Region, u32>, new: &HashMap<Region, u32>) -> bool {
    old.len() != new.len() || old.iter().any(|(r, p)| new.get(r) != Some(p))
}

pub fn escape_markdown(text: &str) -> String {
    text.chars()
        .flat_map(|c| match c {
            '_' | '*' | '~' | '`' => vec!['\\', c],
            _ => vec![c],
        })
        .collect()
}

pub fn format_prices_with_flags(prices: &HashMap<Region, u32>) -> String {
    let mut price_entries: Vec<_> = prices.iter().collect();
    price_entries.sort_by_key(|(r, _)| Region::VARIANTS.iter().position(|v| v == *r).unwrap_or(usize::MAX));

    match price_entries.as_slice() {
        [(region, price)] => format!("{} {price}", region.flag()),
        entries
            if entries.len() == Region::VARIANTS.len()
                && entries.iter().all(|(_, p)| **p == *entries[0].1) =>
        {
            format!(":earth_americas: {}", entries[0].1)
        }
        entries => entries
            .iter()
            .map(|(r, p)| format!("{} {p}", r.flag()))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

pub fn item_header(emoji: &str, title: &str) -> String {
    format!("{emoji} {title}")
}

pub fn format_price_line(prices: &HashMap<Region, u32>) -> String {
    format!(
        "*Price:* {EMOJI_COOKIES} {}",
        format_prices_with_flags(prices)
    )
}

pub fn item_description(desc: &str) -> String {
    if desc.is_empty() {
        String::new()
    } else {
        format!("_{}_\n", escape_markdown(desc))
    }
}

pub fn buy_button(url: &impl ToString) -> String {
    format!("<{}|*{EMOJI_TROLLEY} Buy*>", url.to_string())
}

pub fn format_accessories(accessories: &[Accessory]) -> String {
    if accessories.is_empty() {
        "_none_".to_string()
    } else {
        accessories
            .iter()
            .map(|a| {
                format!(
                    "{} ({EMOJI_COOKIES} {})",
                    escape_markdown(&a.name),
                    format_prices_with_flags(&a.prices)
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub fn format_stock(stock: Option<u32>) -> String {
    match stock {
        Some(0) => "Out of stock".to_string(),
        Some(n) => format!("{n} left"),
        None => "Unlimited".to_string(),
    }
}

pub fn format_achievement_lock(achievement_lock: Option<String>) -> String {
    match achievement_lock {
        Some(s) if s == "Cooking".to_string() => "_Cooking (Black Market)_".to_string(),
        Some(s) if !s.is_empty() => format!("{}", escape_markdown(s.as_str())),
        _ => "_none_".to_string(),
    }
}

pub fn format_long_description(desc: Option<&String>) -> String {
    match desc {
        Some(s) if !s.is_empty() => format!("_{}_", escape_markdown(s)),
        _ => "_none_".to_string(),
    }
}

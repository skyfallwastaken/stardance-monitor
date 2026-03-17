use std::collections::HashMap;

use crate::config::CONFIG;
use crate::format::*;
use crate::scraper::{ShopItem, ShopItems};
use color_eyre::Result;
use log::{debug, info};
use slack_morphism::prelude::*;

fn render_new_item(item: &ShopItem) -> Vec<SlackBlock> {
    let achievement_line = format!(
        "{EMOJI_MEDAL} *Requires achievement:* {}\n",
        format_achievement_lock(item.achievement_lock.clone())
    );

    let section_text = format!(
        "{}{}\n*Stock:* {}\n{achievement_line}\n{}",
        item_description(&item.description),
        format_price_line(&item.prices),
        format_stock(item.remaining_stock),
        buy_button(&item.buy_link())
    );

    vec![
        SlackHeaderBlock::new(pt!(item_header(EMOJI_NEW, &item.title))).into(),
        SlackSectionBlock::new().with_text(md!(section_text)).into(),
        SlackImageBlock::new(
            item.image_url.clone().into(),
            format!("Image for {}", item.title),
        )
        .into(),
    ]
}

fn render_deleted_item(item: &ShopItem) -> Vec<SlackBlock> {
    let section_text = format!(
        "{}{}\n",
        item_description(&item.description),
        format_price_line(&item.prices)
    );

    vec![
        SlackHeaderBlock::new(pt!(item_header(EMOJI_TRASH, &item.title))).into(),
        SlackSectionBlock::new().with_text(md!(section_text)).into(),
        SlackImageBlock::new(
            item.image_url.clone().into(),
            format!("Image for {}", item.title),
        )
        .into(),
    ]
}

fn summarize_long_description_change(
    item_title: &str,
    old_desc: Option<&String>,
    new_desc: Option<&String>,
) -> Option<String> {
    let api_key = CONFIG.openai_api_key.as_ref()?;
    let model = CONFIG.openai_model.as_ref()?;
    let base_url = CONFIG.openai_base_url.as_ref()?;

    let old_text = old_desc.map(|s| s.as_str()).unwrap_or("(empty)");
    let new_text = new_desc.map(|s| s.as_str()).unwrap_or("(empty)");

    let prompt = format!(
        "An item called \"{item_title}\" in a shop had its description changed.\n\n\
         OLD DESCRIPTION:\n{old_text}\n\n\
         NEW DESCRIPTION:\n{new_text}\n\n\
         Write 1-2 sentences summarizing what specifically changed. \
         Be concrete: mention specific names, numbers, specs, vendors, etc. that were added, removed, or changed. \
         Do NOT say \"the description was updated\" - say WHAT changed. \
         Keep it short."
    );

    let url = format!(
        "{}chat/completions",
        base_url.as_str().trim_end_matches('/').to_owned() + "/"
    );

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "max_tokens": 150,
        "temperature": 0.3
    });

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .ok()?;

    if !response.status().is_success() {
        log::warn!("OpenAI API returned status {}", response.status());
        return None;
    }

    let json: serde_json::Value = response.json().ok()?;
    json["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.trim().to_string())
}

fn render_updated_item(old: &ShopItem, new: &ShopItem) -> Vec<SlackBlock> {
    let title = if old.title != new.title {
        format!("{} → {}", old.title, new.title)
    } else {
        new.title.clone()
    };

    let price_line = if prices_changed(&old.prices, &new.prices) {
        format!(
            "*Price:*\n_before:_ {EMOJI_COOKIES} {}\n_after:_ {EMOJI_COOKIES} {}",
            format_prices_with_flags(&old.prices),
            format_prices_with_flags(&new.prices)
        )
    } else {
        format_price_line(&new.prices)
    };

    let description = match (old.description.is_empty(), new.description.is_empty()) {
        (true, true) => String::new(),
        (false, false) if old.description == new.description => item_description(&new.description),
        _ => {
            let old_desc = if old.description.is_empty() {
                "_no description_".to_string()
            } else {
                old.description.clone()
            };
            let new_desc = if new.description.is_empty() {
                "_no description_".to_string()
            } else {
                new.description.clone()
            };
            format!("{old_desc} → {new_desc}\n")
        }
    };

    let long_desc_line = if old.long_description != new.long_description {
        match summarize_long_description_change(
            &new.title,
            old.long_description.as_ref(),
            new.long_description.as_ref(),
        ) {
            Some(s) => format!("*Long Description:* {}\n", escape_markdown(&s)),
            None => format!(
                "*Long Description:* {} → {}\n",
                format_long_description(old.long_description.as_ref()),
                format_long_description(new.long_description.as_ref())
            ),
        }
    } else {
        String::new()
    };

    let accessories_line = if old.accessories != new.accessories {
        format!(
            "*Accessories:* {} → {}\n",
            format_accessories(&old.accessories),
            format_accessories(&new.accessories)
        )
    } else {
        String::new()
    };

    let stock_line = if old.remaining_stock != new.remaining_stock {
        format!(
            "*Stock:* {} → {}\n",
            format_stock(old.remaining_stock),
            format_stock(new.remaining_stock)
        )
    } else {
        format!("*Stock:* {}\n", format_stock(new.remaining_stock))
    };

    let has_achievement = |lock: &Option<String>| lock.as_ref().is_some_and(|s| !s.is_empty());
    let achievement_line = if old.achievement_lock != new.achievement_lock {
        format!(
            "{EMOJI_MEDAL} *Requires achievement:* {} → {}\n",
            format_achievement_lock(old.achievement_lock.clone()),
            format_achievement_lock(new.achievement_lock.clone())
        )
    } else if has_achievement(&new.achievement_lock) {
        format!(
            "{EMOJI_MEDAL} *Requires achievement:* {}\n",
            format_achievement_lock(new.achievement_lock.clone())
        )
    } else {
        String::new()
    };

    let section_text = format!(
        "{description}{price_line}\n{long_desc_line}{accessories_line}{stock_line}{achievement_line}\n{}",
        buy_button(&new.buy_link())
    );

    let mut blocks = vec![
        SlackHeaderBlock::new(pt!(title)).into(),
        SlackSectionBlock::new().with_text(md!(section_text)).into(),
    ];

    if old.image_url != new.image_url {
        blocks.push(
            SlackImageBlock::new(
                old.image_url.clone().into(),
                format!("Old image for {}", new.title),
            )
            .into(),
        );
    }

    blocks.push(
        SlackImageBlock::new(
            new.image_url.clone().into(),
            format!("New image for {}", new.title),
        )
        .into(),
    );
    blocks
}

fn render_channel_ping() -> Vec<SlackBlock> {
    vec![SlackContextBlock::new(vec![SlackContextBlockElement::MarkDown(md!(format!(
        "pinging <!here> · <https://github.com/skyfallwastaken/flavortown-tracker|{EMOJI_STAR} star the repo!> · <https://hackclub.slack.com/archives/C091UF79VDM|{EMOJI_ROBOT} discord/slackbot ysws>"
    )))]).into()]
}

#[derive(Debug)]
pub struct ItemDiff {
    pub new_items: Vec<ShopItem>,
    pub deleted_items: Vec<ShopItem>,
    pub updated_items: Vec<(ShopItem, ShopItem)>,
}

impl ItemDiff {
    pub const fn is_empty(&self) -> bool {
        self.new_items.is_empty() && self.deleted_items.is_empty() && self.updated_items.is_empty()
    }
}

pub fn compute_diff(old_items: &ShopItems, new_items: &ShopItems) -> ItemDiff {
    let old_map: HashMap<_, _> = old_items.iter().map(|i| (i.id, i)).collect();
    let new_map: HashMap<_, _> = new_items.iter().map(|i| (i.id, i)).collect();

    let mut diff = ItemDiff {
        new_items: new_items
            .iter()
            .filter(|item| !old_map.contains_key(&item.id))
            .cloned()
            .collect(),
        deleted_items: old_items
            .iter()
            .filter(|item| !new_map.contains_key(&item.id))
            .cloned()
            .collect(),
        updated_items: Vec::new(),
    };

    diff.updated_items = new_items
        .iter()
        .filter_map(|new_item| {
            old_map
                .get(&new_item.id)
                .filter(|&&old_item| old_item != new_item)
                .map(|old_item| ((*old_item).clone(), new_item.clone()))
        })
        .collect();

    diff
}

const MAX_BLOCKS_PER_MESSAGE: usize = 50;

fn send_blocks(blocks: Vec<SlackBlock>, fallback_text: &str) -> Result<()> {
    use crate::scraper::CLIENT;

    let payload = SlackMessageContent::new()
        .with_text(fallback_text.to_string())
        .with_blocks(blocks);

    debug!(
        "Sending payload: {}",
        serde_json::to_string_pretty(&payload).unwrap_or_default()
    );

    let response = CLIENT
        .post(CONFIG.webhook_url.clone())
        .json(&payload)
        .send()?;

    let status = response.status();
    let body = response.text().unwrap_or_default();
    if !status.is_success() {
        return Err(color_eyre::eyre::eyre!(
            "Slack API error {}: {}",
            status,
            body
        ));
    }

    Ok(())
}

pub fn send_webhook_notifications(diff: &ItemDiff) -> Result<()> {
    let mut item_block_groups: Vec<Vec<SlackBlock>> = Vec::new();

    for item in &diff.new_items {
        info!("Sending notification for new item: {}", item.title);
        item_block_groups.push(render_new_item(item));
    }

    for (old_item, new_item) in &diff.updated_items {
        info!("Sending notification for updated item: {}", new_item.title);
        item_block_groups.push(render_updated_item(old_item, new_item));
    }

    for item in &diff.deleted_items {
        info!("Sending notification for deleted item: {}", item.title);
        item_block_groups.push(render_deleted_item(item));
    }

    let fallback_text = format!(
        "Shop update: {} new, {} updated, {} removed",
        diff.new_items.len(),
        diff.updated_items.len(),
        diff.deleted_items.len()
    );

    let mut current_blocks: Vec<SlackBlock> = Vec::new();

    for (i, group) in item_block_groups.into_iter().enumerate() {
        let group_size = group.len() + 1; // +1 for divider

        if !current_blocks.is_empty()
            && current_blocks.len() + group_size > MAX_BLOCKS_PER_MESSAGE - 1
        {
            send_blocks(current_blocks, &fallback_text)?;
            current_blocks = Vec::new();
        }

        current_blocks.extend(group);
        if i < diff.new_items.len() + diff.updated_items.len() + diff.deleted_items.len() - 1 {
            current_blocks.push(SlackDividerBlock::new().into());
        }
    }

    current_blocks.extend(render_channel_ping());
    send_blocks(current_blocks, &fallback_text)?;

    info!("Successfully sent webhook notifications");
    Ok(())
}

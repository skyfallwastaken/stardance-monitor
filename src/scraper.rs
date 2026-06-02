use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::config::CONFIG;
use crate::storage::{CDN_CACHE_DB, upload_to_cdn};
use color_eyre::{Result, eyre::eyre};
use log::debug;
use once_cell::sync::Lazy;
use rayon::prelude::*;
use reqwest::blocking::Client;
use reqwest::{StatusCode, Url, header, redirect};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use strum::VariantArray;
use strum_macros::{Display, VariantArray};

pub static CLIENT: Lazy<Client> = Lazy::new(|| {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::COOKIE,
        CONFIG
            .cookie
            .parse()
            .expect("cookie parsing failed - check your COOKIE env var"),
    );
    Client::builder()
        .user_agent(&CONFIG.user_agent)
        .default_headers(headers)
        .redirect(redirect::Policy::none())
        .build()
        .expect("failed to build scraping client")
});

#[derive(Display, Debug, VariantArray, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum Region {
    #[strum(to_string = "United States")]
    UnitedStates,
    #[strum(to_string = "EU")]
    Europe,
    #[strum(to_string = "United Kingdom")]
    UnitedKingdom,
    #[strum(to_string = "India")]
    India,
    #[strum(to_string = "Canada")]
    Canada,
    #[strum(to_string = "Australia")]
    Australia,
    #[strum(to_string = "Rest of World")]
    Global,
}

impl Region {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnitedStates => "US",
            Self::Europe => "EU",
            Self::UnitedKingdom => "UK",
            Self::India => "IN",
            Self::Canada => "CA",
            Self::Australia => "AU",
            Self::Global => "XX",
        }
    }

    pub const fn flag(&self) -> &'static str {
        match self {
            Self::UnitedStates => ":flag-us:",
            Self::Europe => ":flag-eu:",
            Self::UnitedKingdom => ":flag-gb:",
            Self::India => ":flag-in:",
            Self::Canada => ":flag-ca:",
            Self::Australia => ":flag-au:",
            Self::Global => ":earth_americas:",
        }
    }
}

pub type ShopItems = Vec<ShopItem>;
pub type ShopItemId = usize;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Accessory {
    pub id: usize,
    pub name: String,
    pub prices: HashMap<Region, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct ShopItem {
    pub title: String,
    pub description: String,
    pub prices: HashMap<Region, u32>,
    pub image_url: Url,

    pub image_id: usize,
    pub id: ShopItemId,

    #[serde(default)]
    pub long_description: Option<String>,
    #[serde(default)]
    pub accessories: Vec<Accessory>,
    #[serde(default)]
    pub remaining_stock: Option<u32>,
    pub achievement_lock: Option<String>,
}

impl ShopItem {
    pub fn buy_link(&self) -> Url {
        CONFIG
            .base_url
            .join(&format!("shop/items/{}", self.id))
            .unwrap()
    }
}

fn select_one<'a>(element: &'a ElementRef, selector: &str) -> Result<ElementRef<'a>> {
    element
        .select(&Selector::parse(selector).unwrap())
        .next()
        .ok_or_else(|| eyre!("missing element: {}", selector))
}

fn parse_shop_item(element: ElementRef, region: &Region) -> Result<ShopItem> {
    let title = element
        .attr("data-shop-wishlist-item-name-value")
        .map(String::from)
        .or_else(|| {
            select_one(&element, ".shop-item-card__title")
                .ok()
                .map(|e| e.text().collect::<String>().trim().to_string())
        })
        .or_else(|| select_one(&element, "h4").ok().map(|e| e.inner_html()))
        .ok_or_else(|| eyre!("missing item title"))?;
    let description = select_one(&element, "div.shop-item-card__description > p")
        .or_else(|_| select_one(&element, "div.shop-item-card__description"))
        .map(|el| crate::mrkdwn::html_to_mrkdwn(el))
        .unwrap_or_default();
    let price: u32 = element
        .attr("data-shop-wishlist-item-price-value")
        .and_then(|v| v.parse().ok())
        .or_else(|| {
            select_one(&element, "span.shop-item-card__price")
                .ok()
                .and_then(|e| {
                    e.text()
                        .collect::<String>()
                        .chars()
                        .filter(char::is_ascii_digit)
                        .collect::<String>()
                        .parse()
                        .ok()
                })
        })
        .or_else(|| {
            select_one(&element, ".shop-item-card__order-cta .action-btn__label")
                .ok()
                .and_then(|e| {
                    e.text()
                        .collect::<String>()
                        .chars()
                        .filter(char::is_ascii_digit)
                        .collect::<String>()
                        .parse()
                        .ok()
                })
        })
        .ok_or_else(|| eyre!("missing price for item"))?;
    let image_url: Url = element
        .attr("data-shop-wishlist-item-image-value")
        .and_then(|v| CONFIG.base_url.join(v).ok())
        .or_else(|| {
            select_one(&element, "img.shop-item-card__image")
                .ok()
                .and_then(|e| e.attr("src"))
                .and_then(|s| CONFIG.base_url.join(s).ok())
        })
        .or_else(|| {
            select_one(&element, "div.shop-item-card__image > img")
                .ok()
                .and_then(|e| e.attr("src"))
                .and_then(|s| s.parse().ok())
        })
        .ok_or_else(|| eyre!("missing image url"))?;
    let image_id = crate::rails::get_rails_blob_id(&image_url)?;
    let id = element
        .attr("data-shop-id")
        .ok_or_else(|| eyre!("missing item id"))?
        .parse()?;

    let mut prices = HashMap::new();
    prices.insert(region.clone(), price);

    Ok(ShopItem {
        title,
        description,
        id,
        image_url,
        image_id,
        prices,
        long_description: None,
        accessories: Vec::new(),
        remaining_stock: None,
        achievement_lock: None,
    })
}

fn fetch_shop_page() -> Result<String> {
    let res = CLIENT
        .get(CONFIG.base_url.join("shop/category/all")?)
        .send()?
        .error_for_status()?;
    assert_eq!(res.status(), StatusCode::OK);
    res.text().map_err(Into::into)
}

fn fetch_item_detail_page(item_id: ShopItemId) -> Result<String> {
    let url = CONFIG.base_url.join(&format!("shop/items/{item_id}"))?;
    let res = CLIENT.get(url).send()?.error_for_status()?;
    assert_eq!(res.status(), StatusCode::OK);
    res.text().map_err(Into::into)
}

struct ItemDetails {
    price: u32,
    long_description: Option<String>,
    accessories: Vec<Accessory>,
    remaining_stock: Option<u32>,
    achievement_lock: Option<String>,
}

fn scrape_item_page_details_for_region(
    item_id: ShopItemId,
    region: &Region,
) -> Result<ItemDetails> {
    let html = fetch_item_detail_page(item_id)?;
    let document = Html::parse_document(&html);
    let root = document.root_element();

    let price: u32 = select_one(&root, "[data-order-form-base-ticket-cost-value]")?
        .attr("data-order-form-base-ticket-cost-value")
        .ok_or_else(|| eyre!("missing base ticket cost for item {item_id}"))?
        .parse()?;

    let long_description = select_one(&root, ".markdown-content")
        .ok()
        .map(|elem| crate::mrkdwn::html_to_mrkdwn(elem))
        .filter(|s| !s.is_empty());

    let remaining_stock = select_one(&root, ".shop-order__stock-indicator span")
        .ok()
        .and_then(|elem| {
            let text = elem.text().collect::<String>();
            if text.contains("Out of stock") {
                Some(0)
            } else {
                text.chars()
                    .filter(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .ok()
            }
        });

    let achievement_lock = select_one(&root, "span.shop-order__achievement-name")
        .ok()
        .map(|elem| {
            let text = elem.text().collect::<String>();
            // let text = text.trim();
            // text.replace("Requires: ", "").trim().to_string()
            text.trim().to_string()
        });

    let label_selector = Selector::parse(".shop-order__accessory-option-label").unwrap();
    let input_selector = Selector::parse(".shop-order__accessory-option-input").unwrap();
    let name_selector = Selector::parse(".shop-order__accessory-option-name").unwrap();
    let mut accessories = Vec::new();

    for label in document.select(&label_selector) {
        let input = label.select(&input_selector).next();
        let name_elem = label.select(&name_selector).next();

        if let (Some(input), Some(name_elem)) = (input, name_elem)
            && let (Some(id_str), Some(price_str)) = (input.attr("value"), input.attr("data-price"))
            && let (Ok(id), Ok(price_f)) = (id_str.parse::<usize>(), price_str.parse::<f64>())
        {
            let price = price_f as u32;
            let name = name_elem.text().collect::<String>().trim().to_string();
            if !accessories.iter().any(|a: &Accessory| a.id == id) {
                let mut prices = HashMap::new();
                prices.insert(region.clone(), price);
                accessories.push(Accessory { id, name, prices });
            }
        }
    }

    Ok(ItemDetails {
        price,
        long_description,
        accessories,
        remaining_stock,
        achievement_lock,
    })
}

fn merge_item_details(item: &mut ShopItem, details: ItemDetails, region: &Region) {
    item.prices.insert(region.clone(), details.price);

    if item.long_description.is_none() {
        item.long_description = details.long_description;
    }

    if item.remaining_stock.is_none() {
        item.remaining_stock = details.remaining_stock;
    }

    if item.achievement_lock.is_none() {
        item.achievement_lock = details.achievement_lock;
    }

    for accessory in details.accessories {
        if let Some(existing) = item.accessories.iter_mut().find(|a| a.id == accessory.id) {
            existing.prices.extend(accessory.prices);
        } else {
            item.accessories.push(accessory);
        }
    }
}

fn get_csrf_token() -> Result<String> {
    let document = Html::parse_document(&fetch_shop_page()?);
    document
        .select(&Selector::parse("meta[name=\"csrf-token\"]").unwrap())
        .next()
        .and_then(|e| e.attr("content"))
        .map(String::from)
        .ok_or_else(|| eyre!("Failed to find csrf-token"))
}

fn set_region(region: &Region, csrf_token: &str) -> Result<()> {
    let res = CLIENT
        .patch(CONFIG.base_url.join("shop/region")?)
        .header("X-CSRF-Token", csrf_token)
        .form(&[("region", region.code())])
        .send()?
        .error_for_status()?;
    assert_eq!(res.status(), StatusCode::OK);
    Ok(())
}

fn scrape_region(region: &Region, csrf_token: &str) -> Result<ShopItems> {
    set_region(region, csrf_token)?;

    let document = Html::parse_document(&fetch_shop_page()?);
    let root = document.root_element();

    // step 1: region selection
    let selected_region = select_one(
        &root,
        "div.shop-category__filters > div.dropdown:nth-of-type(4) > select.dropdown__select > option[selected]",
    )?
    .text()
    .next()
    .unwrap();
    assert_eq!(selected_region, region.to_string());

    // step 2: parse all shop items
    document
        .select(&Selector::parse(".shop-item-card").unwrap())
        .map(|element_ref| parse_shop_item(element_ref, region))
        .collect()
}

pub fn scrape() -> Result<Vec<ShopItem>> {
    let mut items: HashMap<ShopItemId, ShopItem> = HashMap::new();
    let mut items_with_accessories: HashSet<ShopItemId> = HashSet::new();
    let csrf_token = get_csrf_token()?;

    for region in Region::VARIANTS {
        debug!("Now scraping {region:?}");
        let region_items = scrape_region(region, &csrf_token)?;

        let new_item_ids: HashSet<ShopItemId> = region_items
            .iter()
            .filter(|item| !items.contains_key(&item.id))
            .map(|item| item.id)
            .collect();

        for item in &region_items {
            let regional_price = item.prices[region];
            items
                .entry(item.id)
                .and_modify(|existing| {
                    existing.prices.insert(region.clone(), regional_price);
                })
                .or_insert_with(|| item.clone());
        }

        let ids_to_fetch: Vec<ShopItemId> = region_items
            .iter()
            .map(|i| i.id)
            .filter(|id| new_item_ids.contains(id) || items_with_accessories.contains(id))
            .collect();

        let details: Vec<_> = ids_to_fetch
            .par_iter()
            .map(|&id| scrape_item_page_details_for_region(id, region).map(|d| (id, d)))
            .collect::<Result<Vec<_>>>()?;

        for (id, detail) in details {
            if !detail.accessories.is_empty() {
                items_with_accessories.insert(id);
            }
            merge_item_details(items.get_mut(&id).unwrap(), detail, region);
        }
    }

    items
        .par_iter_mut()
        .try_for_each(|(_, item)| -> Result<()> {
            item.image_url = upload_to_cdn(item.image_id, &item.image_url.clone())?;
            item.accessories.sort_by_key(|a| a.id);
            Ok(())
        })?;

    CDN_CACHE_DB.flush()?;

    let mut items = items.into_values().collect::<ShopItems>();
    items.sort_by_key(|item| item.id);
    Ok(items)
}

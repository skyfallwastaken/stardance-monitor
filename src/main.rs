use color_eyre::Result;
use log::{info, warn};
use std::{thread, time::Duration};

mod config;
mod diff;
mod format;
mod mrkdwn;
mod rails;
mod scraper;
mod storage;

const PRICE_CHANGE_CONFIRMATION_DELAY: Duration = Duration::from_secs(45);

fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    color_eyre::install()?;
    env_logger::init();

    let _sentry_guard = config::CONFIG.sentry_dsn.as_ref().map(|dsn| {
        sentry::init((
            dsn.as_str(),
            sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            },
        ))
    });

    info!("Starting scrape job...");
    let items = scraper::scrape()?;
    let old_snap = storage::load_latest_snapshot()?;

    match old_snap {
        Some(old_snap) => {
            let mut items = items;
            let mut item_diff = diff::compute_diff(&old_snap, &items);

            if item_diff.has_price_changes() {
                info!(
                    "Price changes detected; waiting {} seconds and re-scraping before notifying",
                    PRICE_CHANGE_CONFIRMATION_DELAY.as_secs()
                );

                thread::sleep(PRICE_CHANGE_CONFIRMATION_DELAY);
                items = scraper::scrape()?;
                item_diff = diff::compute_diff(&old_snap, &items);
            }

            if item_diff.is_empty() {
                info!("Items haven't changed - exiting!");
                return Ok(());
            }

            info!(
                "*stardance updates:* {} new, {} updated, {} deleted items",
                item_diff.new_items.len(),
                item_diff.updated_items.len(),
                item_diff.deleted_items.len()
            );

            diff::send_webhook_notifications(&item_diff)?;
            storage::write_new_snapshot(&items)?;
        }
        None => {
            warn!("No old snapshot found, writing first snapshot and exiting");
            storage::write_new_snapshot(&items)?;
        }
    }

    Ok(())
}

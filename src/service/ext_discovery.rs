use crate::service::device_database;
use crate::service::extension::Extension;
use crate::service::state::StateHandle;
use async_trait::async_trait;
use std::sync::atomic::{AtomicU64, Ordering};

/// Periodically re-discovers devices via Platform and Undocumented APIs
/// and updates the persistent device database.
pub struct DiscoveryExtension {
    /// Tick counter — only run discovery every N ticks (~30s each)
    tick_count: AtomicU64,
    /// Run discovery every this many ticks (600s / 30s = 20 ticks)
    interval_ticks: u64,
}

impl DiscoveryExtension {
    pub fn new() -> Self {
        Self {
            tick_count: AtomicU64::new(0),
            // Discovery every ~10 minutes (20 ticks * 30s = 600s)
            interval_ticks: 20,
        }
    }
}

#[async_trait]
impl Extension for DiscoveryExtension {
    fn name(&self) -> &str {
        "discovery"
    }

    async fn tick(&self, state: &StateHandle) -> anyhow::Result<()> {
        let count = self.tick_count.fetch_add(1, Ordering::Relaxed);
        if count % self.interval_ticks != 0 || count == 0 {
            // Skip — not time yet, and skip the very first tick (startup already discovered)
            return Ok(());
        }

        log::trace!("Periodic device re-discovery");

        // Re-discover via Platform API
        if let Some(client) = state.get_platform_client().await {
            match client.get_devices().await {
                Ok(devices) => {
                    for info in devices {
                        state
                            .device_mut(&info.sku, &info.device)
                            .await
                            .set_http_device_info(info);
                    }
                }
                Err(err) => {
                    log::error!("Periodic platform API discovery failed: {err:#}");
                }
            }
        }

        // Update persistent device database
        let devices = state.devices().await;
        if !devices.is_empty() {
            let mut db = device_database::load_device_database();
            device_database::update_database_from_devices(&mut db, &devices);
            if let Err(err) = device_database::save_device_database(&db) {
                log::warn!("Failed to save device database: {err:#}");
            }
        }

        Ok(())
    }
}

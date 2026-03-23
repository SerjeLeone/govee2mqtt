use crate::service::extension::Extension;
use crate::service::state::StateHandle;
use async_trait::async_trait;

/// Periodically checks the device config file for changes and reloads it.
pub struct ConfigReloadExtension;

#[async_trait]
impl Extension for ConfigReloadExtension {
    fn name(&self) -> &str {
        "config-reload"
    }

    async fn tick(&self, state: &StateHandle) -> anyhow::Result<()> {
        if crate::service::device_config::check_for_reload() {
            state
                .event_bus
                .emit(crate::service::event_bus::Event::ConfigReloaded);

            // Config changed — re-register entities with HA to pick up new names/settings
            if let Some(hass) = state.get_hass_client().await {
                log::info!("Config reloaded, re-registering entities with Home Assistant");
                if let Err(err) = hass.re_register(state).await {
                    log::warn!("Failed to re-register after config reload: {err:#}");
                }
            }
        }
        Ok(())
    }
}

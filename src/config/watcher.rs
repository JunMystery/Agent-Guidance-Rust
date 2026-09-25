use std::fs;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime};
use tokio::sync::watch;
use tracing::{info, warn};

use super::schema::AppConfig;
use super::store::{config_path, load_config, save_config};

static GLOBAL_WATCHER: OnceLock<ConfigWatcher> = OnceLock::new();

pub struct ConfigWatcher {
    tx: watch::Sender<Arc<AppConfig>>,
    rx: watch::Receiver<Arc<AppConfig>>,
}

impl ConfigWatcher {
    pub fn init() -> &'static Self {
        GLOBAL_WATCHER.get_or_init(|| {
            let initial = load_config().unwrap_or_default();
            let (tx, rx) = watch::channel(Arc::new(initial));
            let watcher = Self { tx, rx };
            watcher.spawn_poll_thread();
            watcher
        })
    }

    pub fn global() -> &'static Self {
        Self::init()
    }

    pub fn current(&self) -> Arc<AppConfig> {
        self.rx.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Arc<AppConfig>> {
        self.rx.clone()
    }

    pub fn update(&self, new_config: AppConfig) -> anyhow::Result<()> {
        save_config(&new_config)?;
        let arc_config = Arc::new(new_config);
        let _ = self.tx.send_replace(arc_config);
        Ok(())
    }

    fn spawn_poll_thread(&self) {
        let tx = self.tx.clone();
        let path = config_path();

        std::thread::Builder::new()
            .name("ag-config-watcher".into())
            .spawn(move || {
                let mut last_modified: Option<SystemTime> =
                    fs::metadata(&path).and_then(|m| m.modified()).ok();

                loop {
                    std::thread::sleep(Duration::from_millis(1500));

                    let current_modified =
                        fs::metadata(&path).and_then(|m| m.modified()).ok();

                    if current_modified != last_modified && current_modified.is_some() {
                        last_modified = current_modified;
                        match load_config() {
                            Ok(fresh) => {
                                info!("Config file changed on disk, reloading settings");
                                let _ = tx.send_replace(Arc::new(fresh));
                            }
                            Err(e) => {
                                warn!("Failed to reload modified config file: {}", e);
                            }
                        }
                    }
                }
            })
            .ok();
    }
}

use crate::Config;
use anyhow::Result;
use directories::ProjectDirs;
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

const CONFIG_FILE: &str = "config.toml";

pub struct AppConfig {
    dirs: Option<ProjectDirs>,
    pub config: Rc<RefCell<Config>>,
}

impl AppConfig {
    pub fn load() -> Self {
        let dirs = ProjectDirs::from("org", "upsuper", "video-sampler");
        let config = dirs
            .as_ref()
            .and_then(|dirs| read_config(&dirs).ok())
            .unwrap_or_default();
        let config = Rc::new(RefCell::new(config));
        Self { dirs, config }
    }
}

impl Drop for AppConfig {
    fn drop(&mut self) {
        if let Some(dirs) = &self.dirs {
            let config = self.config.borrow();
            match write_config(&dirs, &*config) {
                Ok(()) => {}
                Err(e) => eprintln!("failed to write config: {:?}", e),
            }
        }
    }
}

fn read_config(dirs: &ProjectDirs) -> Result<Config> {
    let config_file = dirs.config_dir().join(CONFIG_FILE);
    let data = &fs::read(&config_file)?;
    let config = toml::from_slice(data)?;
    Ok(config)
}

fn write_config(dirs: &ProjectDirs, config: &Config) -> Result<()> {
    let data = toml::to_vec(&config)?;
    fs::create_dir_all(dirs.config_dir())?;
    let config_file = dirs.config_dir().join(CONFIG_FILE);
    fs::write(&config_file, &data)?;
    Ok(())
}

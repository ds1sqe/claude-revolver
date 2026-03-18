use anyhow::Result;

use crate::config::Config;
use crate::util::print_info;

pub fn show() -> Result<()> {
    let config = Config::load()?;
    println!("{}", serde_json::to_string_pretty(&config)?);
    Ok(())
}

pub fn set(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load()?;
    config.set_value(key, value)?;
    config.save()?;
    print_info(&format!("set {key} = {value}"));
    Ok(())
}

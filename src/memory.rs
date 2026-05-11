use crate::world::World;
use anyhow::Result;
use std::{fs, path::Path};

pub fn load(path: &str) -> Result<Option<World>> {
    if !Path::new(path).exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(path)?;
    let world = serde_json::from_str::<World>(&raw)?;
    Ok(Some(world))
}

pub fn save(path: &str, world: &World) -> Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }

    let raw = serde_json::to_string_pretty(world)?;
    fs::write(path, raw)?;
    Ok(())
}

pub fn wipe(path: &str) -> Result<()> {
    if Path::new(path).exists() {
        fs::remove_file(path)?;
    }

    Ok(())
}

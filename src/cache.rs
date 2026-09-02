use crate::github::Snapshot;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

fn path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("gh-assigned").join("snapshot.json"))
}

pub fn load() -> Option<Snapshot> {
    let bytes = fs::read(path()?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn store(snapshot: &Snapshot) -> Result<()> {
    let Some(p) = path() else { return Ok(()) };
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir)?;
    }
    // Write-then-rename so a concurrent reader never sees a truncated file.
    let tmp = p.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec(snapshot)?)?;
    fs::rename(tmp, p)?;
    Ok(())
}

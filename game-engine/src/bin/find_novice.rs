use game_engine::infrastructure::ro_formats::grf::GrfFile;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let grf_path = PathBuf::from("assets/data.grf");
    let grf = GrfFile::from_path(grf_path)?;

    println!("Searching for Novice (초보자) ACT files...\n");

    let novice_files: Vec<_> = grf
        .entries
        .iter()
        .filter(|e| e.filename.contains("초보자") && e.filename.ends_with(".act"))
        .collect();

    println!("Found {} Novice ACT files:", novice_files.len());
    for entry in &novice_files {
        println!("  {}", entry.filename);
    }

    Ok(())
}

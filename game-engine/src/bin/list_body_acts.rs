use game_engine::infrastructure::ro_formats::grf::GrfFile;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let grf_path = PathBuf::from("assets/data.grf");

    println!("Loading GRF file: {}", grf_path.display());
    let grf = GrfFile::from_path(grf_path)?;

    println!("Searching for ACT files containing '몸통'...\n");

    let matching_files: Vec<&str> = grf
        .entries
        .iter()
        .map(|entry| entry.filename.as_str())
        .filter(|filename| filename.contains("몸통") && filename.to_lowercase().ends_with(".act"))
        .collect();

    if matching_files.is_empty() {
        println!("No matching files found.");
    } else {
        println!("Found {} matching files total\n", matching_files.len());
        println!("Showing first 20:\n");
        for (i, filename) in matching_files.iter().take(20).enumerate() {
            println!("{}. {}", i + 1, filename);
        }
    }

    Ok(())
}

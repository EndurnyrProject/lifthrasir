use game_engine::infrastructure::ro_formats::{act::parse_act, grf::GrfFile};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let grf_path = PathBuf::from("assets/data.grf");

    println!("Loading GRF archive: {}", grf_path.display());

    let grf = GrfFile::from_path(grf_path)?;

    println!("Total files in GRF: {}", grf.entries.len());
    println!("\nSearching for base male body ACT file...");

    let target_file = "data\\sprite\\인간족\\몸통\\남\\초보자_남.act";
    let body_acts: Vec<_> = grf
        .entries
        .iter()
        .filter(|e| e.filename == target_file)
        .collect();

    println!("Found {} matching files:", body_acts.len());
    for entry in &body_acts {
        println!("  - {}", entry.filename);
    }

    if body_acts.is_empty() {
        return Err("Base male body ACT file not found".into());
    }

    let act_file_path = &body_acts[0].filename;
    println!("\nExtracting ACT file: {}", act_file_path);

    let act_data = grf
        .get_file(act_file_path)
        .ok_or_else(|| format!("Failed to extract ACT file: {}", act_file_path))?;

    println!("Parsing ACT file...\n");

    let ro_action = parse_act(&act_data)?;

    println!("ACT Version: {}", ro_action.version);
    println!("Total Actions: {}", ro_action.actions.len());
    println!("\nFirst 80 actions:");

    let limit = ro_action.actions.len().min(80);
    for (index, action) in ro_action.actions.iter().enumerate().take(limit) {
        println!(
            "Action {}: {} frames, {}ms delay",
            index,
            action.animations.len(),
            action.delay as u32
        );
    }

    Ok(())
}

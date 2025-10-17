use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use game_engine::infrastructure::ro_formats::GrfFile;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "grf-utils")]
#[command(about = "A CLI utility for extracting and inspecting GRF archive files", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all files in the GRF archive
    List {
        /// Path to the GRF file
        grf_file: PathBuf,
    },
    /// Extract files from the GRF archive
    Extract {
        /// Path to the GRF file
        grf_file: PathBuf,

        /// Files to extract (if empty, extracts all files)
        #[arg(value_name = "FILE")]
        files: Vec<String>,

        /// Output directory (default: "output")
        #[arg(short, long, default_value = "output")]
        output: PathBuf,
    },
    /// Show information about the GRF archive
    Info {
        /// Path to the GRF file
        grf_file: PathBuf,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::List { grf_file } => {
            let grf = load_grf(&grf_file)?;
            list_files(&grf);
        }
        Commands::Extract {
            grf_file,
            files,
            output,
        } => {
            let grf = load_grf(&grf_file)?;
            extract_files(&grf, &files, &output)?;
        }
        Commands::Info { grf_file } => {
            let grf = load_grf(&grf_file)?;
            show_info(&grf);
        }
    }

    Ok(())
}

fn load_grf(path: &Path) -> Result<GrfFile> {
    GrfFile::from_path(path.to_path_buf())
        .with_context(|| format!("Failed to load GRF file: {}", path.display()))
}

fn list_files(grf: &GrfFile) {
    println!("Files in archive:");
    println!("{:-<80}", "");

    for entry in &grf.entries {
        let size = if entry.real_size < 1024 {
            format!("{} B", entry.real_size)
        } else if entry.real_size < 1024 * 1024 {
            format!("{:.2} KB", entry.real_size as f64 / 1024.0)
        } else {
            format!("{:.2} MB", entry.real_size as f64 / (1024.0 * 1024.0))
        };

        println!("{:<60} {:>15}", entry.filename, size);
    }

    println!("{:-<80}", "");
    println!("Total files: {}", grf.entries.len());
}

fn extract_files(grf: &GrfFile, files: &[String], output_path: &Path) -> Result<()> {
    // Create and canonicalize output directory for path traversal protection
    fs::create_dir_all(output_path).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_path.display()
        )
    })?;

    let canonical_output = output_path.canonicalize().with_context(|| {
        format!(
            "Failed to canonicalize output path: {}",
            output_path.display()
        )
    })?;

    if files.is_empty() {
        // Extract all files
        extract_all_files(grf, &canonical_output)?;
    } else {
        // Extract specific files
        extract_specific_files(grf, files, &canonical_output)?;
    }

    Ok(())
}

fn extract_all_files(grf: &GrfFile, canonical_output: &Path) -> Result<()> {
    let entries_count = grf.entries.len() as u64;

    println!("Extracting {} files...", entries_count);

    let pb = ProgressBar::new(entries_count);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) - {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut extracted_count = 0;
    let mut skipped_count = 0;

    for entry in &grf.entries {
        // Normalize path (convert backslashes to forward slashes)
        let normalized_path = entry.filename.replace('\\', "/");
        pb.set_message(normalized_path.clone());

        let output_file_path = canonical_output.join(&normalized_path);

        // SECURITY: Validate path to prevent directory traversal
        match output_file_path.canonicalize() {
            Ok(canonical_file) => {
                if !canonical_file.starts_with(canonical_output) {
                    eprintln!(
                        "Warning: Skipping potentially malicious file path '{}'",
                        entry.filename
                    );
                    skipped_count += 1;
                    pb.inc(1);
                    continue;
                }
            }
            Err(_) => {
                // File doesn't exist yet, check parent directory
                if let Some(parent) = output_file_path.parent() {
                    if !parent.starts_with(canonical_output) {
                        eprintln!(
                            "Warning: Skipping potentially malicious file path '{}'",
                            entry.filename
                        );
                        skipped_count += 1;
                        pb.inc(1);
                        continue;
                    }
                }
            }
        }

        if let Some(data) = grf.get_file(&entry.filename) {
            // Create parent directories
            if let Some(parent) = output_file_path.parent() {
                fs::create_dir_all(parent).ok();
            }

            // Write file
            match fs::write(&output_file_path, data) {
                Ok(_) => extracted_count += 1,
                Err(e) => {
                    eprintln!(
                        "Failed to write file '{}': {}",
                        output_file_path.display(),
                        e
                    );
                    skipped_count += 1;
                }
            }
        } else {
            skipped_count += 1;
        }

        pb.inc(1);
    }

    pb.finish_with_message("Extraction complete");

    println!("\nSummary:");
    println!("  Extracted: {}", extracted_count);
    if skipped_count > 0 {
        println!("  Skipped:   {}", skipped_count);
    }

    Ok(())
}

fn extract_specific_files(grf: &GrfFile, files: &[String], canonical_output: &Path) -> Result<()> {
    println!("Extracting {} specific file(s)...", files.len());

    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} - {msg}",
            )
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut extracted_count = 0;
    let mut not_found_count = 0;

    for file_name in files {
        pb.set_message(file_name.clone());

        // Normalize file name for lookup (GRF uses backslashes)
        let normalized_name = file_name.replace('/', "\\");

        if let Some(data) = grf.get_file(&normalized_name) {
            // Use the user-provided name for output (with forward slashes)
            let output_file_path = canonical_output.join(file_name);

            // SECURITY: Validate path to prevent directory traversal
            match output_file_path.canonicalize() {
                Ok(canonical_file) => {
                    if !canonical_file.starts_with(canonical_output) {
                        eprintln!(
                            "Warning: Skipping potentially malicious file path '{}'",
                            file_name
                        );
                        pb.inc(1);
                        continue;
                    }
                }
                Err(_) => {
                    // File doesn't exist yet, check parent directory
                    if let Some(parent) = output_file_path.parent() {
                        if !parent.starts_with(canonical_output) {
                            eprintln!(
                                "Warning: Skipping potentially malicious file path '{}'",
                                file_name
                            );
                            pb.inc(1);
                            continue;
                        }
                    }
                }
            }

            // Create parent directories
            if let Some(parent) = output_file_path.parent() {
                fs::create_dir_all(parent).ok();
            }

            // Write file
            match fs::write(&output_file_path, data) {
                Ok(_) => {
                    extracted_count += 1;
                    println!("  ✓ {}", file_name);
                }
                Err(e) => {
                    eprintln!("  ✗ Failed to write '{}': {}", file_name, e);
                }
            }
        } else {
            eprintln!("  ✗ File not found in archive: '{}'", file_name);
            not_found_count += 1;
        }

        pb.inc(1);
    }

    pb.finish_with_message("Extraction complete");

    println!("\nSummary:");
    println!("  Extracted: {}", extracted_count);
    if not_found_count > 0 {
        println!("  Not found: {}", not_found_count);
    }

    Ok(())
}

fn show_info(grf: &GrfFile) {
    println!("GRF Archive Information:");
    println!("{:=<80}", "");
    println!("Total files:    {}", grf.entries.len());

    // Calculate total sizes
    let total_compressed: u64 = grf.entries.iter().map(|e| e.pack_size as u64).sum();
    let total_uncompressed: u64 = grf.entries.iter().map(|e| e.real_size as u64).sum();

    println!(
        "Compressed:     {:.2} MB",
        total_compressed as f64 / (1024.0 * 1024.0)
    );
    println!(
        "Uncompressed:   {:.2} MB",
        total_uncompressed as f64 / (1024.0 * 1024.0)
    );

    let compression_ratio = if total_uncompressed > 0 {
        (total_compressed as f64 / total_uncompressed as f64) * 100.0
    } else {
        0.0
    };
    println!("Compression:    {:.1}%", compression_ratio);

    // File type statistics
    let encrypted_count = grf
        .entries
        .iter()
        .filter(|e| e.file_type & 0x06 != 0)
        .count();
    if encrypted_count > 0 {
        println!("Encrypted:      {} files", encrypted_count);
    }

    println!("{:=<80}", "");
}

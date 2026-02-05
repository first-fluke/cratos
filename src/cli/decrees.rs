//! Decrees CLI commands
//!
//! `cratos decrees` - View and manage decrees (laws)

use super::{DecreeType, DecreesCommands};
use anyhow::Result;
use cratos_core::decrees::DecreeLoader;

/// Run decrees command
pub async fn run(cmd: DecreesCommands) -> Result<()> {
    match cmd {
        DecreesCommands::Show { decree } => show(&decree).await,
        DecreesCommands::Validate => validate().await,
    }
}

/// Show a decree document
async fn show(decree_type: &DecreeType) -> Result<()> {
    let loader = DecreeLoader::new();

    match decree_type {
        DecreeType::Laws => show_laws(&loader).await,
        DecreeType::Ranks => show_ranks(&loader).await,
        DecreeType::Warfare => show_warfare(&loader).await,
    }
}

/// Show laws
async fn show_laws(loader: &DecreeLoader) -> Result<()> {
    if !loader.laws_exists() {
        println!("⚠️  Laws not found: config/decrees/laws.toml");
        println!();
        print_laws_template();
        return Ok(());
    }

    match loader.load_laws() {
        Ok(laws) => {
            println!("{}", laws.format_display());
        }
        Err(e) => {
            println!("⚠️  Failed to load laws: {e}");
        }
    }

    Ok(())
}

/// Show ranks
async fn show_ranks(loader: &DecreeLoader) -> Result<()> {
    if !loader.ranks_exists() {
        println!("⚠️  Ranks not found: config/decrees/ranks.toml");
        println!();
        println!("  Create ranks.toml with rank definitions.");
        return Ok(());
    }

    match loader.load_ranks() {
        Ok(ranks) => {
            println!("{}", ranks.format_display());
        }
        Err(e) => {
            println!("⚠️  Failed to load ranks: {e}");
        }
    }

    Ok(())
}

/// Show warfare rules
async fn show_warfare(loader: &DecreeLoader) -> Result<()> {
    if !loader.warfare_exists() {
        println!("⚠️  Warfare not found: config/decrees/warfare.toml");
        println!();
        println!("  Create warfare.toml with development rules.");
        return Ok(());
    }

    match loader.load_warfare() {
        Ok(warfare) => {
            println!("{}", warfare.format_display());
        }
        Err(e) => {
            println!("⚠️  Failed to load warfare: {e}");
        }
    }

    Ok(())
}

/// Validate decree compliance
async fn validate() -> Result<()> {
    println!("\n⚖️  Validating decree compliance...\n");

    let loader = DecreeLoader::new();
    let result = loader.validate_all();

    // Laws check
    if let Some(count) = result.laws_count {
        if result.laws_valid {
            println!("  ✅ Laws ({count} articles): Valid");
        } else {
            println!("  ⚠️  Laws ({count} articles): 10 or more recommended");
        }
    } else if let Some(err) = &result.laws_error {
        println!("  ❌ Laws: Error - {err}");
    } else {
        println!("  ❌ Laws: Not found");
    }

    // Ranks check
    if let Some(count) = result.ranks_count {
        if result.ranks_valid {
            println!("  ✅ Ranks ({count} ranks): Valid");
        } else {
            println!("  ⚠️  Ranks ({count} ranks): 8 or more recommended");
        }
    } else if let Some(err) = &result.ranks_error {
        println!("  ⚠️  Ranks: Error - {err}");
    } else {
        println!("  ⚠️  Ranks: Not found (optional)");
    }

    // Warfare check
    if let Some(count) = result.warfare_count {
        if result.warfare_valid {
            println!("  ✅ Warfare ({count} sections): Valid");
        } else {
            println!("  ⚠️  Warfare: Empty sections");
        }
    } else if let Some(err) = &result.warfare_error {
        println!("  ⚠️  Warfare: Error - {err}");
    } else {
        println!("  ⚠️  Warfare: Not found (optional)");
    }

    println!();
    if result.all_valid() {
        println!("✅ All decrees are valid.");
    } else if result.required_valid() {
        println!("✅ Required decrees (Laws) are valid.");
    } else {
        println!("⚠️  Some decrees need attention.");
    }
    println!();

    Ok(())
}

/// Print default laws template
fn print_laws_template() {
    println!("  Default laws.toml template:");
    println!();
    println!(
        r#"  [meta]
  title = "Laws (LAWS)"
  philosophy = "Command with logic, act with persona, prove with records"
  immutable = true

  [[articles]]
  id = 1
  title = "Planning and Design"
  rules = ["All features must have a 1-Pager specification document"]

  [[articles]]
  id = 2
  title = "Development Guidelines"
  rules = ["Report architecture before implementation", "Strictly follow Clean Architecture"]
"#
    );
}

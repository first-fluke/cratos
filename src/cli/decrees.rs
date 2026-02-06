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
        DecreeType::Alliance => show_extended(&loader, "alliance").await,
        DecreeType::Tribute => show_extended(&loader, "tribute").await,
        DecreeType::Judgment => show_extended(&loader, "judgment").await,
        DecreeType::Culture => show_extended(&loader, "culture").await,
        DecreeType::Operations => show_extended(&loader, "operations").await,
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

/// Show an extended decree (alliance, tribute, judgment, culture, operations)
async fn show_extended(loader: &DecreeLoader, name: &str) -> Result<()> {
    let exists = match name {
        "alliance" => loader.alliance_exists(),
        "tribute" => loader.tribute_exists(),
        "judgment" => loader.judgment_exists(),
        "culture" => loader.culture_exists(),
        "operations" => loader.operations_exists(),
        _ => false,
    };

    if !exists {
        println!("⚠️  {name} not found: config/decrees/{name}.toml");
        println!();
        println!("  Create {name}.toml with [meta] + [[articles]] structure.");
        return Ok(());
    }

    let result = match name {
        "alliance" => loader.load_alliance(),
        "tribute" => loader.load_tribute(),
        "judgment" => loader.load_judgment(),
        "culture" => loader.load_culture(),
        "operations" => loader.load_operations(),
        _ => unreachable!(),
    };

    match result {
        Ok(decree) => {
            println!("{}", decree.format_display());
        }
        Err(e) => {
            println!("⚠️  Failed to load {name}: {e}");
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

    // Extended decrees
    for ext in &result.extended {
        if let Some(count) = ext.count {
            if ext.valid {
                println!("  ✅ {} ({count} articles): Valid", ext.name);
            } else {
                println!("  ⚠️  {} ({count} articles): Empty", ext.name);
            }
        } else if let Some(err) = &ext.error {
            println!("  ⚠️  {}: Error - {err}", ext.name);
        }
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

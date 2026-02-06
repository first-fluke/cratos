//! Prompt helpers with inquire → stdin fallback.
//!
//! Every prompt gracefully degrades: if `inquire` fails (e.g. not a real TTY),
//! we fall back to plain stdin prompts.

use inquire::{Confirm, Password, Select};
use std::io::{self, BufRead, Write};

/// Read a trimmed line from stdin.
fn read_line() -> anyhow::Result<String> {
    let mut input = String::new();
    io::stdin()
        .lock()
        .read_line(&mut input)
        .map_err(|e| anyhow::anyhow!("Failed to read input: {}", e))?;
    Ok(input.trim().to_string())
}

/// Confirm prompt with fallback.
pub fn confirm(message: &str, default: bool, help: Option<&str>) -> anyhow::Result<bool> {
    let mut builder = Confirm::new(message).with_default(default);
    if let Some(h) = help {
        builder = builder.with_help_message(h);
    }
    match builder.prompt() {
        Ok(v) => Ok(v),
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => {
            anyhow::bail!("Cancelled");
        }
        Err(_) => {
            let hint = if default { "Y/n" } else { "y/N" };
            if let Some(h) = help {
                println!("  {}", h);
            }
            print!("? {} ({}) ", message, hint);
            io::stdout().flush()?;
            let input = read_line()?;
            match input.to_lowercase().as_str() {
                "y" | "yes" => Ok(true),
                "n" | "no" => Ok(false),
                _ => Ok(default),
            }
        }
    }
}

/// Password prompt (optional) with fallback.
pub fn password(message: &str) -> anyhow::Result<String> {
    match Password::new(message)
        .with_display_mode(inquire::PasswordDisplayMode::Masked)
        .prompt()
    {
        Ok(v) => Ok(v),
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => {
            anyhow::bail!("Cancelled");
        }
        Err(_) => {
            print!("  {} ", message);
            io::stdout().flush()?;
            read_line()
        }
    }
}

/// Password prompt (required) with fallback.
pub fn password_required(message: &str) -> anyhow::Result<String> {
    match Password::new(message)
        .with_display_mode(inquire::PasswordDisplayMode::Masked)
        .with_validator(inquire::required!())
        .prompt()
    {
        Ok(v) => Ok(v),
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => {
            anyhow::bail!("Cancelled");
        }
        Err(_) => loop {
            print!("  {} ", message);
            io::stdout().flush()?;
            let input = read_line()?;
            if !input.is_empty() {
                return Ok(input);
            }
            println!("  (required)");
        },
    }
}

/// Selection prompt with fallback to numbered list.
pub fn select(message: &str, options: &[String]) -> anyhow::Result<String> {
    match Select::new(message, options.to_vec()).prompt() {
        Ok(v) => Ok(v),
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => {
            anyhow::bail!("Cancelled");
        }
        Err(_) => {
            println!();
            for (i, opt) in options.iter().enumerate() {
                if opt.starts_with("──") {
                    println!("  {}", opt);
                } else {
                    println!("  [{}] {}", i, opt);
                }
            }
            println!();
            loop {
                print!("  {} ", message);
                io::stdout().flush()?;
                let input = read_line()?;
                if let Ok(idx) = input.parse::<usize>() {
                    if idx < options.len() && !options[idx].starts_with("──") {
                        return Ok(options[idx].clone());
                    }
                }
                println!("  (enter a valid number)");
            }
        }
    }
}

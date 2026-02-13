//! Diagnosis formatting

use super::types::Diagnosis;

/// Format diagnosis as user-friendly text
pub fn format_diagnosis(diagnosis: &Diagnosis) -> String {
    let mut output = String::new();

    output.push_str("🔍 **Tool Doctor Diagnosis**\n\n");
    output.push_str(&format!("**Tool:** `{}`\n", diagnosis.tool_name));
    output.push_str(&format!(
        "**Error Category:** {} ({}% confidence)\n\n",
        diagnosis.category.display_name(),
        (diagnosis.confidence * 100.0) as u8
    ));

    output.push_str("**Probable Causes:**\n");
    for (i, cause) in diagnosis.probable_causes.iter().take(3).enumerate() {
        output.push_str(&format!(
            "{}. {} ({}% likely)\n   → {}\n",
            i + 1,
            cause.description,
            cause.likelihood,
            cause.verification
        ));
    }

    output.push_str("\n**Resolution Checklist:**\n");
    for item in &diagnosis.checklist {
        output.push_str(&format!(
            "☐ Step {}: {}\n   ```\n   {}\n   ```\n   Expected: {}\n",
            item.step, item.action, item.instruction, item.expected_result
        ));
    }

    if !diagnosis.alternatives.is_empty() {
        output.push_str("\n**Alternative Approaches:**\n");
        for alt in &diagnosis.alternatives {
            if let Some(tool) = &alt.tool_name {
                output.push_str(&format!("• {} (use `{}`)\n", alt.description, tool));
            } else {
                output.push_str(&format!("• {}\n", alt.description));
            }
            output.push_str(&format!("  Trade-offs: {}\n", alt.tradeoffs));
        }
    }

    output
}

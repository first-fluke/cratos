//! Reusable UI Components
//!
//! This module provides a collection of pure Rust UI components
//! for the Cratos web dashboard.

mod card;
mod chart;
mod code_block;
mod diagram;
mod markdown;
mod message;

pub use card::{Card, StatCard};
pub use chart::{Chart, ChartData, ChartType, DataSeries, SimpleBarChart, SimpleLineChart};
pub use code_block::CodeBlock;
pub use diagram::{Diagram, DiagramType, EditableDiagram};
pub use markdown::MarkdownBlock;
pub use message::MessageBubble;

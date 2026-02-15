//! Page Components

pub mod chat;
mod dashboard;
mod docs;
mod history;
mod memory;
mod personas;
mod settings;
mod tools;

mod history_detail;
pub use history_detail::HistoryDetail;

pub use chat::Chat;
pub use dashboard::Dashboard;
pub use docs::Docs;
pub use history::History;
pub use memory::Memory;
pub use personas::Personas;
pub use settings::Settings;
pub use tools::Tools;


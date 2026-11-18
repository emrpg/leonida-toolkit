pub mod companion;
pub mod core_adapter;
pub mod db;
pub mod error;
pub mod rage;
pub mod routes;
pub mod sol;
pub mod state;

pub use routes::build_router;
pub use state::AppState;

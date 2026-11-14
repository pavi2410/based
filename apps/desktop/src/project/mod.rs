// project/ — `.based/` directory I/O: project load, file watcher, variable resolution.

pub mod context;
pub mod discovery;
pub mod loader;
pub mod open;
pub mod personal;
pub mod pick;
pub mod reload;
pub mod scope;
pub mod settings;
pub mod watcher;

pub use based_query::variables::{load_variables, substitute_dollar_vars as substitute};
pub use context::ProjectContext;
pub use discovery::find_project_root;
pub use open::{
    prompt_open_project_in_new_window, prompt_open_project_in_window,
    request_close_project_in_window,
};

pub use reload::{drain_pending_reload, reload_from_disk};
pub use scope::ProjectState;

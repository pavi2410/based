//! Re-export variable helpers from `based-query`.

pub use based_query::variables::{
    Variables, load_variables, save_variables, substitute_dollar_vars as substitute,
};

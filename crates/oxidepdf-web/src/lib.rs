mod schema;
pub use schema::{FamilySchema, OpMeta};

const INDEX_HTML: &str = include_str!("../static/index.html");
const STYLE_CSS: &str = include_str!("../static/style.css");
const FORM_JS: &str = include_str!("../static/form.js");
const APP_JS: &str = include_str!("../static/app.js");

include!("lib/parse_size.rs");
include!("lib/blocking_io.rs");
include!("lib/run_workflow.rs");
include!("lib/web_security.rs");
include!("lib/api_execute_workflow.rs");

#[cfg(test)]
mod tests;

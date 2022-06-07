use super::super::table::*;

use toml::Value as Toml;

const DEFAULT_ACL_CONFIG: &str = r#"
[default]
out-get = { action = "out-get", access = "accept" }
in = { action = "in-*", group = { location = "inner" }, access = "accept" }
out = { action = "out-*", group = { location = "inner" }, access = "accept" }
"#;

pub(crate) struct AclDefault;

impl AclDefault {
    pub fn load(container: &AclTableContainer) {
        let table = Self::load_table(DEFAULT_ACL_CONFIG);
        container.load(table, true).unwrap();
    }

    fn load_table(content: &str) -> toml::value::Table {
        info!("will load default acl: {}", content);

        let value: Toml = toml::from_str(content).unwrap();

        match value {
            Toml::Table(table) => table,
            _ => {
                unreachable!();
            }
        }
    }
}

pub trait Dialect: Send + Sync {
    fn name(&self) -> &'static str;
    fn identifier_quote(&self) -> char;
    fn placeholder(&self, index: usize) -> String;
    fn supports_returning(&self) -> bool;
    fn supports_on_conflict(&self) -> bool;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PostgresDialect;

impl Dialect for PostgresDialect {
    fn name(&self) -> &'static str {
        "PostgreSQL"
    }

    fn identifier_quote(&self) -> char {
        '"'
    }

    fn placeholder(&self, index: usize) -> String {
        format!("${index}")
    }

    fn supports_returning(&self) -> bool {
        true
    }

    fn supports_on_conflict(&self) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SqliteDialect;

impl Dialect for SqliteDialect {
    fn name(&self) -> &'static str {
        "SQLite"
    }

    fn identifier_quote(&self) -> char {
        '"'
    }

    fn placeholder(&self, _index: usize) -> String {
        "?".to_string()
    }

    fn supports_returning(&self) -> bool {
        true
    }

    fn supports_on_conflict(&self) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MysqlDialect;

impl Dialect for MysqlDialect {
    fn name(&self) -> &'static str {
        "MySQL"
    }

    fn identifier_quote(&self) -> char {
        '`'
    }

    fn placeholder(&self, _index: usize) -> String {
        "?".to_string()
    }

    fn supports_returning(&self) -> bool {
        false
    }

    fn supports_on_conflict(&self) -> bool {
        false
    }
}

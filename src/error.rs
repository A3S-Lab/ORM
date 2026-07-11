#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid SQL identifier: {0:?}")]
    InvalidIdentifier(String),
    #[error("query requires at least one selected expression")]
    EmptySelection,
    #[error("scalar subquery requires exactly one selected expression, found {0}")]
    InvalidScalarSubquery(usize),
    #[error("duplicate common table expression name: {0:?}")]
    DuplicateCte(String),
    #[error("insert query requires at least one value")]
    EmptyInsert,
    #[error("update query requires at least one assignment")]
    EmptyUpdate,
    #[error("insert values belong to table {actual:?}, expected {expected:?}")]
    WrongInsertTable { expected: String, actual: String },
    #[error("update value belongs to table {actual:?}, expected {expected:?}")]
    WrongUpdateTable { expected: String, actual: String },
    #[error("query compilation failed: {0}")]
    Compilation(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

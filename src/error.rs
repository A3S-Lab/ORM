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
    #[error("insert row {row} has columns that differ from the first row")]
    InconsistentInsertColumns { row: usize },
    #[error("insert row {row} assigns column {column:?} more than once")]
    DuplicateInsertColumn { row: usize, column: String },
    #[error("conflict target requires at least one column")]
    EmptyConflictTarget,
    #[error("conflict target requires an action")]
    MissingConflictAction,
    #[error("conflict update requires at least one assignment")]
    EmptyConflictUpdate,
    #[error("conflict clause assigns column {0:?} more than once")]
    DuplicateConflictColumn(String),
    #[error("conflict clause contains a column from table {actual:?}, expected {expected:?}")]
    WrongConflictTable { expected: String, actual: String },
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

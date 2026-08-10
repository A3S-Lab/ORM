use crate::ast::{
    Assignment, ConflictAction, ConflictValue, InsertNode, SelectNode, TableNode, UpdateAssignment,
};
use crate::error::{Error, Result};

fn verify_insert_assignments(table: &TableNode, assignments: &[Assignment]) -> Result<()> {
    if let Some(wrong) = assignments
        .iter()
        .find(|assignment| assignment.table != table.name)
    {
        return Err(Error::WrongInsertTable {
            expected: table.name.to_owned(),
            actual: wrong.table.to_owned(),
        });
    }
    Ok(())
}

pub(super) fn verify_insert_rows(node: &InsertNode) -> Result<()> {
    let first_columns = node.rows[0]
        .iter()
        .map(|assignment| assignment.column)
        .collect::<Vec<_>>();
    for (row_index, row) in node.rows.iter().enumerate() {
        verify_insert_assignments(&node.table, row)?;
        let mut columns = std::collections::HashSet::with_capacity(row.len());
        for assignment in row {
            if !columns.insert(assignment.column) {
                return Err(Error::DuplicateInsertColumn {
                    row: row_index,
                    column: assignment.column.to_owned(),
                });
            }
        }
        if row
            .iter()
            .map(|assignment| assignment.column)
            .ne(first_columns.iter().copied())
        {
            return Err(Error::InconsistentInsertColumns { row: row_index });
        }
    }
    verify_conflict(node)
}

pub(super) fn verify_update_assignments(
    table: &TableNode,
    assignments: &[UpdateAssignment],
) -> Result<()> {
    if let Some(wrong) = assignments
        .iter()
        .find(|assignment| assignment.table != table.name)
    {
        return Err(Error::WrongUpdateTable {
            expected: table.name.to_owned(),
            actual: wrong.table.to_owned(),
        });
    }
    Ok(())
}

fn verify_conflict(node: &InsertNode) -> Result<()> {
    let Some(conflict) = &node.conflict else {
        return Ok(());
    };
    if conflict.target.is_empty() {
        return Err(Error::EmptyConflictTarget);
    }
    let mut target = std::collections::HashSet::with_capacity(conflict.target.len());
    for column in &conflict.target {
        if !target.insert(*column) {
            return Err(Error::DuplicateConflictColumn((*column).to_owned()));
        }
    }
    match &conflict.action {
        None => Err(Error::MissingConflictAction),
        Some(ConflictAction::DoNothing) => Ok(()),
        Some(ConflictAction::DoUpdate(assignments)) => {
            if assignments.is_empty() {
                return Err(Error::EmptyConflictUpdate);
            }
            let mut columns = std::collections::HashSet::with_capacity(assignments.len());
            for assignment in assignments {
                verify_conflict_table(node.table.name, assignment.table)?;
                if let ConflictValue::Excluded { table, .. } = &assignment.value {
                    verify_conflict_table(node.table.name, table)?;
                }
                if !columns.insert(assignment.column) {
                    return Err(Error::DuplicateConflictColumn(assignment.column.to_owned()));
                }
            }
            Ok(())
        }
    }
}

fn verify_conflict_table(expected: &str, actual: &str) -> Result<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(Error::WrongConflictTable {
            expected: expected.to_owned(),
            actual: actual.to_owned(),
        })
    }
}

pub(super) fn validate_identifier(identifier: &str) -> Result<()> {
    if identifier.is_empty()
        || !identifier
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
        || identifier
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
    {
        return Err(Error::InvalidIdentifier(identifier.to_owned()));
    }
    Ok(())
}

pub(super) fn validate_sql_type(sql_type: &str) -> Result<()> {
    validate_identifier(sql_type).map_err(|_| Error::InvalidSqlType(sql_type.to_owned()))
}

pub(super) fn verify_select_lock(node: &SelectNode) -> Result<()> {
    let Some(lock) = &node.lock else {
        return Ok(());
    };
    if !node.set_operations.is_empty() {
        return Err(Error::UnsupportedLockingSetOperation);
    }
    let available = std::iter::once(&node.from)
        .chain(node.joins.iter().map(|join| &join.table))
        .map(|table| table.alias.unwrap_or(table.name))
        .collect::<std::collections::HashSet<_>>();
    for table in &lock.tables {
        if !available.contains(table) {
            return Err(Error::InvalidLockTarget((*table).to_owned()));
        }
    }
    Ok(())
}

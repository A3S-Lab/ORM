use crate::{Row, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct SqliteRow {
    values: Vec<Value>,
}

impl Row for SqliteRow {
    fn value(&self, index: usize) -> Option<&Value> {
        self.get(index)
    }
}

impl SqliteRow {
    pub(crate) fn new(values: Vec<Value>) -> Self {
        Self { values }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&Value> {
        self.values.get(index)
    }

    pub fn into_values(self) -> Vec<Value> {
        self.values
    }
}

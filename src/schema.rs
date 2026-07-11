use std::marker::PhantomData;

pub trait Table: Send + Sync + 'static {
    const NAME: &'static str;
}

#[derive(Debug, Clone, Copy)]
pub struct TableRef<T: Table> {
    alias: Option<&'static str>,
    marker: PhantomData<T>,
}

impl<T: Table> Default for TableRef<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Table> TableRef<T> {
    pub const fn new() -> Self {
        Self {
            alias: None,
            marker: PhantomData,
        }
    }

    pub const fn alias(alias: &'static str) -> Self {
        Self {
            alias: Some(alias),
            marker: PhantomData,
        }
    }

    pub const fn name(&self) -> &'static str {
        T::NAME
    }

    pub const fn alias_name(&self) -> Option<&'static str> {
        self.alias
    }
}

pub trait Store {
    fn get(&self) -> String;
}

pub struct MemStore;

impl Store for MemStore {
    fn get(&self) -> String {
        "mem".to_string()
    }
}

pub enum Kind {
    Alpha,
    Beta,
}

pub const LIMIT: usize = 10;

pub type Alias = String;

#[macro_export]
macro_rules! mk_store {
    () => {
        $crate::store::MemStore
    };
}

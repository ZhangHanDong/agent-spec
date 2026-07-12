pub mod service;
pub mod store;

pub struct Config {
    pub name: String,
}

pub fn open_default() -> store::MemStore {
    store::MemStore
}

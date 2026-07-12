use crate::store::{MemStore, Store};

pub fn run(store: &MemStore) -> String {
    store.get()
}

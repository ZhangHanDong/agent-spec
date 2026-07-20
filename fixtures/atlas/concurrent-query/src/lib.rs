pub struct Store {
    values: Vec<u64>,
}

impl Store {
    pub fn new(values: Vec<u64>) -> Self {
        Self { values }
    }

    pub fn get(&self, index: usize) -> Option<u64> {
        self.values.get(index).copied()
    }
}

pub fn load(store: &Store, index: usize) -> Option<u64> {
    store.get(index)
}

pub fn validate(value: u64) -> bool {
    value > 0
}

pub fn transform(value: u64) -> u64 {
    value.saturating_mul(2)
}

pub fn persist(value: u64) -> u64 {
    value
}

pub fn execute(store: &Store, index: usize) -> Option<u64> {
    let value = load(store, index)?;
    validate(value).then(|| persist(transform(value)))
}

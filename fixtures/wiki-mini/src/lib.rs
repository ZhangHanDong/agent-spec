#[derive(Debug, Default)]
pub struct Counter {
    value: i32,
}

impl Counter {
    pub fn increment(&mut self) {
        self.value += 1;
    }

    pub fn value(&self) -> i32 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::Counter;

    #[test]
    fn counter_increment_adds_one() {
        let mut counter = Counter::default();
        counter.increment();
        assert_eq!(counter.value(), 1);
    }
}

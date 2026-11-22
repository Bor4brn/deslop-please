pub(super) struct Values {
    all: [u32; 25],
}

impl Values {
    pub(super) fn new() -> Values {
        Values { all: [0; 25] }
    }

    pub(super) fn get(&self, index: usize) -> u32 {
        self.all[index]
    }
}

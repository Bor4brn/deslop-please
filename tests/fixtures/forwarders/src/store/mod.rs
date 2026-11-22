mod table;

use table::Table;

pub struct Store {
    table: Table,
}

impl Store {
    pub fn new() -> Store {
        Store { table: Table::new() }
    }

    pub fn a(&self) -> u32 {
        self.table.a()
    }

    pub fn b(&self) -> u32 {
        self.table.b()
    }

    pub fn c(&self) -> u32 {
        self.table.c()
    }

    pub fn d(&self) -> u32 {
        self.table.d()
    }

    pub fn e(&self) -> u32 {
        self.table.e()
    }

    pub fn f(&self) -> u32 {
        self.table.f()
    }

    pub fn g(&self) -> u32 {
        self.table.g()
    }

    pub fn h(&self) -> u32 {
        self.h_from_table()
    }

    fn h_from_table(&self) -> u32 {
        self.table.h()
    }
}

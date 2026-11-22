use crate::store::Store;

pub fn run() -> u32 {
    let store = Store::new();
    store.a() + store.b() + store.c() + store.d() + store.e() + store.f() + store.g() + store.h()
}

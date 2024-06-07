use my_library::{expand, T, MyTrait};

#[ic_cdk::init]
fn init() {
    ic_certified_assets::init();
}

#[ic_cdk::query]
fn greet(name: String) -> String {
    let _t: T = 42.into();  // This cannot locate to the from impl
    let t = T::from(42); // This is okay
    t.chain().unsafe_inner().expect("ERR");
    format!("Hello, {}! {}", name.len(), expand!(name))
}
#[ic_cdk::update]
fn f() -> u8 {
  T::trait_func()
}
#[ic_cdk::update]
fn g() {
  let _ = T + T;
}
use my_library::{expand, MyTrait, T};

#[ic_cdk::init]
fn init() {
    let _: T = 42.into(); // doesn't report unsafe at the moment
}

#[ic_cdk::query]
fn greet(name: String) -> String {
    let t = T::from(42);
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
#[ic_cdk::update]
fn h() -> u64 {
    expand!(my_library::non_ic_func());
    expand!(my_library::stable64_size())
}

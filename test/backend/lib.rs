use my_library::{expand, T};

#[ic_cdk::init]
fn init() {
    ic_certified_assets::init();
}

#[ic_cdk::query]
fn greet(name: String) -> String {
    let t = T;
    t.chain().inner().expect("ERR");
    format!("Hello, {}! {}", expand!(name.clone()), name.len())
}

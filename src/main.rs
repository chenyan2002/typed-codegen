use anyhow::Result;
use std::path::PathBuf;

mod builder;
mod load_cargo;

fn main() -> Result<()> {
    env_logger::init();
    let path = PathBuf::from("/Users/yan.chen/src/examples/rust/basic_dao");
    let (db, vfs, target) = load_cargo::load_cargo_project(&path)?;
    let krate = load_cargo::find_root_crate(&db, &vfs, &target)?;
    let mut builder = builder::Builder::new(&db, krate);
    builder.build();
    println!("{}", builder.emit_methods());
    Ok(())
}

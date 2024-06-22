use anyhow::Result;
use candid::types::{Type, TypeEnv};
use candid_parser::bindings::rust::{compile, Config, ExternalConfig};
use candid_parser::{
    bindings::analysis::project_methods, configs::Configs, utils::CandidSource, Deserialize,
    Principal,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use toml::{Table, Value};

#[derive(Deserialize)]
struct Item {
    path: Option<PathBuf>,
    canister_id: Option<Principal>,
    output_dir: Option<PathBuf>,
    template: Option<String>,
    methods: Option<Vec<String>>,
    bindgen: Option<Table>,
}
#[derive(Deserialize)]
struct Entry {
    service: Option<Item>,
    imports: BTreeMap<String, Item>,
}

pub fn run(path: &Path) -> Result<()> {
    let path = path.join("canister.toml");
    let config = load_toml(&path)?;
    let mut src_dir = &PathBuf::from("./src");
    if let Some(serv) = &config.service {
        if let Some(path) = &serv.output_dir {
            src_dir = path;
        }
        let name = src_dir.join("lib.rs");
        let res = generate_service(serv)?;
        println!("{}\n{}", name.display(), res);
    }
    for (name, item) in &config.imports {
        let path = item.output_dir.as_ref().unwrap_or(src_dir);
        let name = path.join(format!("{}.rs", name));
        let res = generate_import(item)?;
        println!("{}\n{}", name.display(), res);
    }
    Ok(())
}
fn generate_import(item: &Item) -> Result<String> {
    let (env, actor) = load_candid(item)?;
    let (config, external) = get_config(item, "canister_call")?;
    let res = compile(&config, &env, &actor, external);
    Ok(res)
}
fn generate_service(item: &Item) -> Result<String> {
    assert!(item.methods.is_none());
    let (env, actor) = load_candid(item)?;
    let (config, external) = get_config(item, "stub")?;
    let res = compile(&config, &env, &actor, external);
    Ok(res)
}
fn get_config(item: &Item, target: &str) -> Result<(Config, ExternalConfig)> {
    let mut external = ExternalConfig::default();
    if let Some(template) = &item.template {
        external
            .0
            .insert("target".to_string(), "custom".to_string());
        external
            .0
            .insert("template".to_string(), template.to_string());
    } else {
        external.0.insert("target".to_string(), target.to_string());
    }
    let configs = Configs(item.bindgen.clone().unwrap_or_default());
    let config = Config::new(configs);
    Ok((config, external))
}
fn load_candid(item: &Item) -> Result<(TypeEnv, Option<Type>)> {
    let src = if let Some(p) = &item.path {
        CandidSource::File(p)
    } else if let Some(_id) = item.canister_id {
        todo!("canister_id not implemented")
    } else {
        return Err(anyhow::anyhow!("path or canister_id must be provided"));
    };
    let (env, mut actor) = src.load()?;
    match item.methods.as_deref() {
        None => (),
        Some([]) => actor = None,
        Some(methods) => actor = project_methods(&env, &actor, methods),
    }
    Ok((env, actor))
}
fn load_toml(path: &Path) -> Result<Entry> {
    let toml = std::fs::read_to_string(path)?;
    let mut table: Table = toml::from_str(&toml)?;
    let service: Option<Item> = if let Some(v) = table.remove("service") {
        Some(v.try_into()?)
    } else {
        None
    };
    let mut imports = BTreeMap::new();
    if let Some(Value::Table(t)) = table.remove("imports") {
        for (k, v) in t {
            imports.insert(k, v.try_into()?);
        }
    }
    Ok(Entry { service, imports })
}

use anyhow::Result;
use candid::types::{Type, TypeEnv};
use candid_parser::bindings::rust::{compile, Config, ExternalConfig};
use candid_parser::{
    bindings::analysis::project_methods, configs::Configs, utils::CandidSource, Deserialize,
    Principal,
};
use log::{info, warn};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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

pub fn run(path: &Path, write: bool) -> Result<()> {
    let path = path.join("canister.toml");
    let config = load_toml(&path)?;
    let mut src_dir = &PathBuf::from("./src");
    if let Some(serv) = &config.service {
        if serv.methods.is_some() {
            return Err(anyhow::anyhow!("service.methods shouldn't be present"));
        }
        if serv.path.is_none() {
            return Err(anyhow::anyhow!("service.path is required"));
        }
        if let Some(path) = &serv.output_dir {
            src_dir = path;
        }
        let name = src_dir.join("lib.rs");
        if name.exists() {
            info!("Checking main file {} (experimental)", name.display());
            let (config, _) = get_config(serv, "stub")?;
            crate::check::check_rust(&name, &serv.path.clone().unwrap(), &config)?;
        } else {
            info!("Generating main file {}", name.display());
            let res = generate_service(serv)?;
            if write {
                std::fs::write(name, res)?;
            } else {
                info!("\n{}", res);
            }
        }
    }
    for (name, item) in &config.imports {
        let path = item.output_dir.as_ref().unwrap_or(src_dir);
        let name = path.join(format!("{}.rs", name));
        let res = generate_import(item)?;
        info!("Generating import {}", name.display());
        if write {
            std::fs::write(name, res)?;
        } else {
            info!("\n{}", res);
        }
    }
    Ok(())
}
fn generate_import(item: &Item) -> Result<String> {
    let (env, actor) = load_candid(item)?;
    let (config, external) = get_config(item, "canister_call")?;
    let res = compile(&config, &env, &actor, external);
    let res = invoke_rustfmt(res);
    Ok(res)
}
fn generate_service(item: &Item) -> Result<String> {
    let (env, actor) = load_candid(item)?;
    let (config, external) = get_config(item, "stub")?;
    let res = compile(&config, &env, &actor, external);
    let res = invoke_rustfmt(res);
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
    let (env, mut actor) = if let Some(p) = &item.path {
        CandidSource::File(p).load()?
    } else if let Some(id) = item.canister_id {
        // TODO: handle versioning
        info!("Fetching Candid interface from {id}");
        let src = fetch_metadata(id)?;
        CandidSource::Text(&src).load()?
    } else {
        return Err(anyhow::anyhow!("path or canister_id must be provided"));
    };
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
fn invoke_rustfmt(content: String) -> String {
    invoke_rustfmt_(&content).unwrap_or_else(|_| {
        warn!("rustfmt failed, using unformatted code.");
        content
    })
}
fn invoke_rustfmt_(content: &str) -> Result<String> {
    use std::io::Write;
    let mut fmt = Command::new("rustfmt")
        .arg("--edition")
        .arg("2021")
        .arg("--emit")
        .arg("stdout")
        .arg("--quiet")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut stdin = fmt.stdin.take().unwrap();
    let content = content.to_string();
    std::thread::spawn(move || {
        stdin.write_all(content.as_bytes()).unwrap();
    });
    let output = fmt.wait_with_output()?;
    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(anyhow::anyhow!("rustfmt failed"))
    }
}

#[tokio::main]
async fn fetch_metadata(id: Principal) -> Result<String> {
    let agent = ic_agent::Agent::builder()
        .with_url("https://icp0.io")
        .build()?;
    let blob = agent
        .read_state_canister_metadata(id, "candid:service")
        .await?;
    Ok(String::from_utf8(blob)?)
}

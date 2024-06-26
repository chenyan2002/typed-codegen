use crate::utils::create_bar;
use crate::Options;
use anyhow::Result;
use indicatif::{MultiProgress, ProgressBar};
use ra_ap_base_db::CrateId;
use ra_ap_hir::Crate;
use ra_ap_ide::RootDatabase;
use ra_ap_load_cargo::{load_workspace, LoadCargoConfig, ProcMacroServerChoice};
use ra_ap_paths::{AbsPathBuf, Utf8PathBuf};
use ra_ap_project_model::{
    CargoConfig, CargoFeatures, CargoWorkspace, ProjectManifest, ProjectWorkspace,
    ProjectWorkspaceKind, TargetData, TargetKind,
};
use ra_ap_vfs::Vfs;
use std::path::Path;

pub fn load_cargo_project(
    options: &Options,
    bars: &MultiProgress,
) -> Result<(CargoWorkspace, RootDatabase, Vfs, TargetData)> {
    let bar = create_bar(bars, "Loading project...");
    let path = options.manifest_path.as_path();
    let cargo_config = cargo_config(options);
    let load_config = load_config(options);
    let pb = create_bar(bars, "Building...");
    let mut ws = load_project_workspace(path, &cargo_config, &pb)?;
    if load_config.load_out_dirs_from_check {
        let build_scripts = ws.run_build_scripts(&cargo_config, &|msg| {
            pb.set_message(msg.to_string());
        })?;
        ws.set_build_scripts(build_scripts);
    }
    let cargo = match &ws.kind {
        ProjectWorkspaceKind::Cargo { cargo, .. } => cargo.clone(),
        _ => return Err(anyhow::anyhow!("Not a cargo workspace")),
    };
    let target = find_package(&cargo, options.package.as_deref(), None)?;
    let (db, vfs, _proc) = load_workspace(ws, &cargo_config.extra_env, &load_config)?;
    pb.finish_and_clear();
    bar.finish();
    Ok((cargo, db, vfs, target))
}

fn load_project_workspace(
    path: &Path,
    cargo_config: &CargoConfig,
    bar: &ProgressBar,
) -> Result<ProjectWorkspace> {
    let path_buf = std::env::current_dir()?.join(path).canonicalize()?;
    bar.set_message(format!("Loading project workspace: {:?}", path_buf));
    let utf8_path = Utf8PathBuf::from_path_buf(path_buf).unwrap();
    let root = AbsPathBuf::assert(utf8_path);
    let root = ProjectManifest::discover_single(root.as_path())?;
    ProjectWorkspace::load(root, cargo_config, &|msg| {
        bar.set_message(msg.to_string());
    })
}
fn cargo_config(options: &Options) -> CargoConfig {
    let mut config = CargoConfig {
        target: Some("wasm32-unknown-unknown".to_string()),
        // sysroot needs to present for proc macro expansion to work
        sysroot: Some(ra_ap_project_model::RustLibSource::Discover),
        ..Default::default()
    };
    config.features = if options.all_features {
        CargoFeatures::All
    } else {
        CargoFeatures::Selected {
            features: options.features.clone(),
            no_default_features: options.no_default_features,
        }
    };
    config
}
fn load_config(options: &Options) -> LoadCargoConfig {
    LoadCargoConfig {
        load_out_dirs_from_check: options.expand_proc_macros,
        prefill_caches: false,
        with_proc_macro_server: ProcMacroServerChoice::Sysroot,
    }
}
fn find_package(
    cargo: &CargoWorkspace,
    name: Option<&str>,
    version: Option<&str>,
) -> Result<TargetData> {
    let packages: Vec<_> = cargo
        .packages()
        .filter(|idx| {
            let package = &cargo[*idx];
            if let Some(name) = name {
                package.name == *name
                    && (version.is_none()
                        || package.version.to_string() == *version.as_ref().unwrap())
            } else {
                package.is_member
            }
        })
        .collect();
    if packages.len() != 1 {
        if packages.is_empty() {
            let name = if let Some(name) = name {
                name
            } else {
                "in the workspace"
            };
            return Err(anyhow::anyhow!("Cannot find package {}", name));
        }
        if name.is_some() {
            let packages: Vec<_> = packages
                .into_iter()
                .map(|idx| format!("{}@{}", cargo[idx].name, cargo[idx].version))
                .collect();
            return Err(anyhow::anyhow!(
                "Multiple packages found:\n{}",
                packages.join("\n")
            ));
        } else {
            let packages: Vec<_> = packages
                .into_iter()
                .map(|idx| cargo[idx].name.to_string())
                .collect();
            return Err(anyhow::anyhow!(
                "Multiple packages present in workspace, please select one via --package flag:\n{}",
                packages.join("\n")
            ));
        }
    }
    let package_idx = packages[0];
    let package = cargo[package_idx].clone();
    let targets: Vec<_> = package
        .targets
        .iter()
        .cloned()
        .filter(|idx| matches!(&cargo[*idx].kind, TargetKind::Lib { .. }))
        .collect();
    if targets.len() != 1 {
        return Err(anyhow::anyhow!(
            "No library target found for {}.",
            package.name
        ));
    }
    let target = cargo[targets[0]].clone();
    Ok(target)
}
pub fn find_whitelisted_crates(
    ws: &CargoWorkspace,
    db: &RootDatabase,
    vfs: &Vfs,
    whitelist: &[String],
) -> Result<Vec<CrateId>> {
    let mut res = Vec::new();
    for item in whitelist {
        let parsed: Vec<_> = item.split('@').collect();
        let name = parsed[0];
        let version = parsed.get(1);
        let target = match find_package(ws, Some(name), version.copied()) {
            Ok(target) => target,
            Err(e) => {
                if e.to_string().starts_with("Cannot find package") {
                    log::warn!("Cannot find crate {name}, ignoring...");
                    continue;
                } else {
                    return Err(e);
                }
            }
        };
        let krate = find_crate(db, vfs, &target)?;
        res.push(krate.into());
    }
    Ok(res)
}
pub fn find_crate(db: &RootDatabase, vfs: &Vfs, target: &TargetData) -> Result<Crate> {
    let crates = Crate::all(db);
    let root_path = target.root.as_path();
    let krate = crates.into_iter().find(|krate| {
        let vfs_path = vfs.file_path(krate.root_file(db));
        let crate_root_path = vfs_path.as_path().unwrap();
        crate_root_path == root_path
    });
    krate.ok_or_else(|| anyhow::anyhow!("crate {} not found", target.name))
}
pub fn find_non_root_crates(db: &RootDatabase, vfs: &Vfs, target: &TargetData) -> Vec<Crate> {
    use ra_ap_base_db::CrateOrigin;
    let crates = Crate::all(db);
    let root_path = target.root.as_path();
    crates
        .into_iter()
        .filter(|krate| {
            let vfs_path = vfs.file_path(krate.root_file(db));
            let crate_root_path = vfs_path.as_path().unwrap();
            crate_root_path != root_path
                && !matches!(
                    krate.origin(db),
                    CrateOrigin::Rustc { .. } | CrateOrigin::Lang(_)
                )
        })
        .collect()
}

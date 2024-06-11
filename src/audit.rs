use crate::utils::create_bar;
use crate::utils::{crate_name, display_path};
use console::style;
use fxhash::FxHashSet;
use indicatif::MultiProgress;
use log::{info, trace, warn};
use ra_ap_base_db::CrateId;
use ra_ap_hir::{self as hir, Crate, HirDisplay, Semantics};
use ra_ap_hir_def::FunctionId;
use ra_ap_ide::RootDatabase;
use ra_ap_syntax::SyntaxNode;

#[derive(PartialEq)]
pub enum Mode {
    TraceFunctions,
    ScanExports,
}

pub struct Builder<'a> {
    db: &'a RootDatabase,
    krate: Crate,
    semantics: Semantics<'a, RootDatabase>,
    mode: Mode,
    whitelist: Vec<CrateId>,
    bars: &'a MultiProgress,
    pub visited: FxHashSet<FunctionId>,
}
impl<'a> Builder<'a> {
    pub fn new(
        bars: &'a MultiProgress,
        db: &'a RootDatabase,
        krate: Crate,
        whitelist: Vec<CrateId>,
        mode: Mode,
    ) -> Self {
        Self {
            bars,
            db,
            krate,
            mode,
            whitelist,
            semantics: Semantics::new(db),
            visited: FxHashSet::default(),
        }
    }
    pub fn build(&mut self) {
        if self.whitelist.contains(&CrateId::from(self.krate)) {
            return;
        }
        let name = crate_name(self.krate, self.db);
        let name = if let Some(ver) = self.krate.version(self.db) {
            format!("{name} {ver}")
        } else {
            name
        };
        let bar = create_bar(self.bars, format!("Auditing crate {name}..."));
        let module = self.krate.root_module();
        self.process_module(module);
        for impl_ in hir::Impl::all_in_crate(self.db, self.krate) {
            self.process_impl(impl_);
        }
        bar.finish_and_clear();
        self.bars.remove(&bar);
    }
    fn process_module(&mut self, module: hir::Module) {
        trace!("Processing module: {}", module.display(self.db));
        let decls = module.declarations(self.db);
        for d in decls {
            self.process_def(d);
        }
    }
    fn process_def(&mut self, def: hir::ModuleDef) {
        trace!("Processing def: {:?}", def.name(self.db));
        match def {
            hir::ModuleDef::Module(module) => self.process_module(module),
            hir::ModuleDef::Function(func) => self.process_function(func, &mut Vec::new()),
            _ => (),
        }
    }
    fn process_impl(&mut self, impl_: hir::Impl) {
        impl_.items(self.db).into_iter().for_each(|item| {
            if let hir::AssocItem::Function(func) = item {
                self.process_function(func, &mut Vec::new());
            }
        });
    }
    fn process_function(&mut self, func: hir::Function, path: &mut Vec<String>) {
        use ra_ap_base_db::CrateOrigin;
        use ra_ap_hir::{HasAttrs, HasContainer, ItemContainer};
        use ra_ap_syntax::ast::AstNode;
        if !self.visited.insert(func.into()) {
            return;
        }
        let krate = func.module(self.db).krate();
        if matches!(
            krate.origin(self.db),
            CrateOrigin::Rustc { .. } | CrateOrigin::Lang(_)
        ) {
            return;
        }
        let is_whitelisted = self.whitelist.contains(&CrateId::from(krate));
        let name = display_path(func.into(), self.db);
        let bar = create_bar(self.bars, format!("Processing function: {name}..."));
        info!("Processing function: {name}...");
        path.push(name.clone());
        if let ItemContainer::ExternBlock() = func.container(self.db) {
            self.report(
                is_whitelisted,
                path,
                format!(
                    "{} {} is an {} import!",
                    style("[Import]").red().bold(),
                    style(&name).red(),
                    style("external").red()
                ),
            );
        }
        match self.mode {
            Mode::TraceFunctions => {
                let Some(ast) = self.semantics.source(func) else {
                    warn!("cannot get source for {name}");
                    return;
                };
                self.process_syntax_node(&name, is_whitelisted, path, ast.value.syntax());
            }
            Mode::ScanExports => {
                let attrs = func.attrs(self.db);
                if let Some(export) = attrs.export_name() {
                    self.report(
                        is_whitelisted,
                        path,
                        format!(
                            "{} {} exports {}",
                            style("[Export]").yellow().bold(),
                            style(&name).yellow(),
                            style(export).yellow()
                        ),
                    );
                }
            }
        }
        bar.finish_and_clear();
        self.bars.remove(&bar);
        path.pop();
    }
    fn process_syntax_node(
        &mut self,
        name: &str,
        is_whitelisted: bool,
        path: &mut Vec<String>,
        ast: &SyntaxNode,
    ) {
        use ra_ap_hir::{CallableKind, PathResolution};
        use ra_ap_syntax::{ast, match_ast, AstNode};
        for node in ast.descendants() {
            match_ast! {
                match node {
                    ast::MacroCall(m) => if let Some(m) = self.semantics.expand(&m) {
                        self.process_syntax_node(name, is_whitelisted, path, &m);
                    },
                    ast::BlockExpr(b) =>if b.unsafe_token().is_some() {
                        self.report(is_whitelisted, path, format!("{} {} contains {} blocks!", style("[Unsafe]").yellow().bold(), style(name).yellow(), style("unsafe").yellow()));
                    },
                    ast::AwaitExpr(e) => if let Some(f) = self.semantics.resolve_await_to_poll(&e) {
                        self.process_function(f, path);
                    },
                    ast::PrefixExpr(e) => if let Some(f) = self.semantics.resolve_prefix_expr(&e) {
                        self.process_function(f, path);
                    },
                    ast::IndexExpr(e) => if let Some(f) = self.semantics.resolve_index_expr(&e) {
                        self.process_function(f, path);
                    },
                    ast::BinExpr(e) => if let Some(f) = self.semantics.resolve_bin_expr(&e) {
                        self.process_function(f, path);
                    },
                    ast::TryExpr(e) => if let Some(f) = self.semantics.resolve_try_expr(&e) {
                        self.process_function(f, path);
                    },
                    ast::MethodCallExpr(m) => if let Some(call) = self.semantics.resolve_method_call_as_callable(&m) {
                        if let CallableKind::Function(f) = call.kind() {
                            /*
                            // Looking at m's type isn't correct. Need to inspect type parameter in the signature.
                            if let ItemContainer::Trait(t) = f.container(self.db) {
                                if let Some(ty) = self.semantics.type_of_expr(&m.clone().into()) {
                                    let ty = ty.adjusted().display(self.db).to_string();
                                    let impls = hir::Impl::all_for_trait(self.db, t);
                                    if let Some(impl_) = impls.iter().find(|i| i.self_ty(self.db).display(self.db).to_string() == ty) {
                                        let func = impl_.items(self.db).into_iter().find_map(|assoc| {
                                            if let AssocItem::Function(func) = assoc {
                                                if func.name(self.db) == f.name(self.db) {
                                                    return Some(func);
                                                }
                                            };
                                            None
                                        });
                                        if let Some(f) = func {
                                            warn!("{m} : {ty}");
                                            self.process_function(f, path);
                                        }
                                    }
                                }
                            }
                            */
                            self.process_function(f, path);
                        }
                    },
                    ast::PathExpr(path_expr) => if let Some(p) = path_expr.path() {
                        if let Some(PathResolution::Def(hir::ModuleDef::Function(f))) = self.semantics.resolve_path(&p) {
                            self.process_function(f, path);
                            /*
                            // This is an over-approximation. Ideally, we can find the right impl.
                            if let ItemContainer::Trait(t) = f.container(self.db) {
                                let impls = hir::Impl::all_for_trait(self.db, t);
                                let impls: Vec<_> = impls.into_iter().flat_map(|i| i.items(self.db).into_iter().filter_map(|assoc| match assoc {
                                    AssocItem::Function(func) if func.name(self.db) == f.name(self.db) => Some(func),
                                    _ => None,
                                })).collect();
                                //warn!("{} {} => {:?}", node, impls.len(), t);
                                for f in impls {
                                    self.process_function(f, path);
                                }
                            }
                            */
                        }
                    },
                    ast::Expr(e) => if let Some(call) = self.semantics.resolve_expr_as_callable(&e) {
                        if let CallableKind::Function(f) = call.kind() {
                            self.process_function(f, path);
                        }
                    },
                    _ => (),
                }
            }
        }
    }
    fn report(&self, is_whitelisted: bool, path: &[String], msg: impl std::convert::AsRef<str>) {
        if !is_whitelisted {
            self.bars.println(msg).unwrap();
            if path.len() > 1 {
                let path: Vec<String> = path.iter().map(|p| style(p).cyan().to_string()).collect();
                self.bars
                    .println(format!(
                        "  {} {}",
                        style("└────> [Path]").green(),
                        path.join(" -> ")
                    ))
                    .unwrap();
            }
        }
    }
}

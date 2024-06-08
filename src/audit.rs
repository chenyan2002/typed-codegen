use crate::utils::{crate_name, display_path};
use fxhash::FxHashSet;
use log::{debug, error, info, trace, warn};
use ra_ap_base_db::CrateId;
use ra_ap_hir::{self as hir, Crate, HirDisplay, Semantics};
use ra_ap_hir_def::FunctionId;
use ra_ap_ide::RootDatabase;
use ra_ap_syntax::SyntaxNode;

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
    pub visited: FxHashSet<FunctionId>,
}
impl<'a> Builder<'a> {
    pub fn new(db: &'a RootDatabase, krate: Crate, whitelist: Vec<CrateId>, mode: Mode) -> Self {
        Self {
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
        info!("Auditing crate {}...", name);
        let module = self.krate.root_module();
        self.process_module(module);
        for impl_ in hir::Impl::all_in_crate(self.db, self.krate) {
            self.process_impl(impl_);
        }
        debug!("Found {} functions", self.visited.len());
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
            hir::ModuleDef::Function(func) => self.process_function(func),
            _ => (),
        }
    }
    fn process_impl(&mut self, impl_: hir::Impl) {
        impl_.items(self.db).into_iter().for_each(|item| {
            if let hir::AssocItem::Function(func) = item {
                self.process_function(func);
            }
        });
    }
    fn process_function(&mut self, func: hir::Function) {
        use ra_ap_base_db::CrateOrigin;
        use ra_ap_hir::{HasAttrs, HasContainer, ItemContainer};
        use ra_ap_syntax::ast::AstNode;
        if !self.visited.insert(func.into()) {
            return;
        }
        let krate = func.module(self.db).krate();
        if self.whitelist.contains(&CrateId::from(krate)) {
            return;
        }
        if matches!(
            krate.origin(self.db),
            CrateOrigin::Rustc { .. } | CrateOrigin::Lang(_)
        ) {
            return;
        }
        let name = display_path(func.into(), self.db);
        trace!("Processing function: {name}");
        if let ItemContainer::ExternBlock() = func.container(self.db) {
            error!("{name} is an external imports!");
        }
        match self.mode {
            Mode::TraceFunctions => {
                let Some(ast) = self.semantics.source(func) else {
                    warn!("cannot get source for {name}");
                    return;
                };
                self.process_syntax_node(&name, ast.value.syntax());
            }
            Mode::ScanExports => {
                let attrs = func.attrs(self.db);
                if let Some(export) = attrs.export_name() {
                    error!("{} exports {}", name, export);
                }
            }
        }
    }
    fn process_syntax_node(&mut self, name: &str, ast: &SyntaxNode) {
        use ra_ap_hir::{AsAssocItem, CallableKind};
        use ra_ap_syntax::{ast, match_ast, AstNode};
        for node in ast.descendants() {
            match_ast! {
                match node {
                    ast::MacroCall(m) => if let Some(m) = self.semantics.expand(&m) {
                        self.process_syntax_node(name, &m);
                    },
                    ast::BlockExpr(b) => if b.unsafe_token().is_some() {
                        warn!("{name} contains unsafe block!");
                        for stmt in b.statements() {
                            self.process_syntax_node(name, stmt.syntax());
                        }
                        if let Some(e) = b.tail_expr() {
                            self.process_syntax_node(name, e.syntax());
                        }
                    },
                    ast::MethodCallExpr(m) => if let Some(call) = self.semantics.resolve_method_call_as_callable(&m) {
                        if let CallableKind::Function(f) = call.kind() {
                            self.process_function(f);
                        }
                    },
                    ast::AwaitExpr(e) => if let Some(f) = self.semantics.resolve_await_to_poll(&e) {
                        self.process_function(f);
                    },
                    ast::PrefixExpr(e) => if let Some(f) = self.semantics.resolve_prefix_expr(&e) {
                        self.process_function(f);
                    },
                    ast::IndexExpr(e) => if let Some(f) = self.semantics.resolve_index_expr(&e) {
                        self.process_function(f);
                    },
                    ast::BinExpr(e) => if let Some(f) = self.semantics.resolve_bin_expr(&e) {
                        self.process_function(f);
                    },
                    ast::TryExpr(e) => if let Some(f) = self.semantics.resolve_try_expr(&e) {
                        self.process_function(f);
                    },
                    ast::Expr(e) => if let Some(call) = self.semantics.resolve_expr_as_callable(&e) {
                        if let CallableKind::Function(f) = call.kind() {
                            if let Some(assoc) = f.as_assoc_item(self.db) {
                                let container = assoc.container(self.db);
                                debug!("{} => {:?}", f.display(self.db), container);
                            }
                            self.process_function(f);
                        }
                    },
                    _ => (),
                }
            }
        }
    }
}

use log::{debug, trace};
use ra_ap_hir::{self as hir, Crate, HirDisplay};
use ra_ap_ide::RootDatabase;

pub struct Builder<'a> {
    db: &'a RootDatabase,
    krate: Crate,
}
impl<'a> Builder<'a> {
    pub fn new(db: &'a RootDatabase, krate: Crate) -> Self {
        Self { db, krate }
    }
    pub fn build(&self) {
        trace!("Scanning project...");
        let module = self.krate.root_module();
        self.process_module(module);
    }
    fn process_module(&self, module: hir::Module) {
        trace!("Processing module: {}", module.display(self.db));
        let decls = module.declarations(self.db);
        for d in decls {
            self.process_def(d);
        }
    }
    fn process_def(&self, def: hir::ModuleDef) {
        trace!("Processing def: {:?}", def.name(self.db));
        match def {
            hir::ModuleDef::Module(module) => self.process_module(module),
            hir::ModuleDef::Function(func) => self.process_function(func),
            hir::ModuleDef::Adt(adt) => self.process_adt(adt),
            _ => (),
        }
    }
    fn process_function(&self, func: hir::Function) {
        trace!("Processing function: {}", func.display(self.db));
        debug!("Function: {}", func.ty(self.db).display(self.db));
    }
    fn process_adt(&self, adt: hir::Adt) {
        trace!("Processing adt: {adt:?}");
        match adt {
            hir::Adt::Struct(s) => debug!("Struct: {}", s.display(self.db)),
            hir::Adt::Enum(e) => debug!("Enum: {}", e.display(self.db)),
            hir::Adt::Union(_) => (),
        }
        for impl_ in hir::Impl::all_for_type(self.db, adt.ty(self.db)) {
            self.process_impl(impl_);
        }
    }
    fn process_impl(&self, impl_: hir::Impl) {
        for item in impl_.items(self.db) {
            match item {
                hir::AssocItem::Function(f) => self.process_function(f),
                _ => (),
            }
        }
    }
}

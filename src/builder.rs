use log::trace;
use ra_ap_hir::{self as hir, Crate, HirDisplay};
use ra_ap_ide::RootDatabase;

pub struct Builder<'a> {
    db: &'a RootDatabase,
    krate: Crate,
    pub methods: Vec<hir::Function>,
}
impl<'a> Builder<'a> {
    pub fn new(db: &'a RootDatabase, krate: Crate) -> Self {
        Self {
            db,
            krate,
            methods: Vec::new(),
        }
    }
    pub fn build(&mut self) {
        trace!("Scanning project...");
        let module = self.krate.root_module();
        self.process_module(module)
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
            hir::ModuleDef::Adt(_) => (),
            _ => (),
        }
    }
    fn process_function(&mut self, func: hir::Function) {
        trace!("Processing function: {}", func.display(self.db));
        let cdk_attr = get_cdk_attr(&func, self.db);
        if cdk_attr.is_some() {
            self.methods.push(func);
        } else {
            trace!(
                "{} is not a CDK function",
                func.name(self.db).as_str().unwrap()
            );
        }
    }
    pub fn emit_methods(&self) -> String {
        use std::io::Write;
        let mut res = Vec::new();
        for func in &self.methods {
            let attr = get_cdk_attr(func, self.db).unwrap();
            let mode = if attr.mode == "query" { " query" } else { "" };
            let name = func.name(self.db);
            let name = name.as_str().unwrap();
            let args = func.params_without_self(self.db);
            let args = args.iter().map(|p| p.ty()).collect::<Vec<_>>();
            let args = args
                .into_iter()
                .map(|t| t.display(self.db).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let ret = if func.is_async(self.db) {
                func.async_ret_type(self.db).unwrap()
            } else {
                func.ret_type(self.db)
            };
            let ret = ret.display(self.db);
            writeln!(&mut res, "{name} : ({args}) -> ({ret}){mode};").unwrap();
        }
        String::from_utf8_lossy(&res).to_string()
    }
}
struct CDKAttr {
    mode: String,
}
fn get_cdk_attr(func: &hir::Function, db: &RootDatabase) -> Option<CDKAttr> {
    use ra_ap_hir::HasAttrs;
    let attrs = func.attrs(db);
    let cdk = attrs.iter().find(|attr| {
        let attr = attr.path().segments().last().map(|s| s.to_smol_str());
        attr == Some("update".into()) || attr == Some("query".into()) || attr == Some("init".into())
    })?;
    let mode = cdk.path().segments().last()?.as_str().unwrap().to_string();
    Some(CDKAttr { mode })
}

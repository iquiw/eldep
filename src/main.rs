#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate solvent;

use std::io::{BufRead, BufReader};
use std::fs::File;
use std::path::{Path, PathBuf};

use regex::Regex;
use solvent::DepGraph;

struct Feature(String);

impl Feature {
    fn to_path_buf(&self) -> PathBuf {
        let mut path = PathBuf::from(&self.0);
        path.set_extension("el");
        path
    }
}

fn main() {
    if let Err(err) = resolve_dependencies(".") {
        eprintln!("{}", err);
    }
}

fn resolve_dependencies<P>(dir: P) -> Result<(), Box<std::error::Error>>
where
    P: AsRef<Path>,
{
    let mut elisps = vec![];
    for entry in dir.as_ref().read_dir()? {
        let path = entry?.path();
        if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
            if ext == "el" {
                elisps.push(path.to_path_buf());
            }
        }
    }
    let depgraph = gather_dependencies(&elisps)?;
    for elisp in elisps {
        let mut deps = vec![];
        for d in depgraph.dependencies_of(&elisp)? {
            let path = d?;
            if path == &elisp {
                continue;
            }
            if let Some(el) = path.to_str() {
                deps.push(el);
            }
        }
        println!("{}: {}", elisp.display(), &deps[..].join(" "));
    }
    Ok(())
}

fn gather_dependencies<P>(elisps: &Vec<P>) -> Result<DepGraph<PathBuf>, Box<std::error::Error>>
where
    P: AsRef<Path>,
{
    let mut depgraph = DepGraph::new();
    for elisp in elisps {
        match extract_requires(elisp) {
            Ok(v) => {
                if v.is_empty() {
                    depgraph.register_node(elisp.as_ref().to_path_buf());
                }
                for f in v {
                    depgraph.register_dependency(elisp.as_ref().to_path_buf(), f.to_path_buf());
                }
            }
            Err(err) => eprintln!("{}", err),
        }
    }
    Ok(depgraph)
}

fn extract_requires<P>(path: P) -> Result<Vec<Feature>, Box<std::error::Error>>
where
    P: AsRef<Path>,
{
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^\(require '([\w-]+)\)").unwrap();
    }
    let f = File::open(path)?;
    let reader = BufReader::new(f);
    let mut v = vec![];
    for r in reader.lines() {
        let l = r?;
        if let Some(f) = RE.captures(&l).and_then(|c| c.get(1)) {
            v.push(Feature(f.as_str().to_string()));
        }
    }
    Ok(v)
}

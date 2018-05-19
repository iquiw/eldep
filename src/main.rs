#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate solvent;
extern crate tabwriter;

use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use regex::Regex;
use solvent::DepGraph;
use tabwriter::TabWriter;

struct Feature(String);

impl Feature {
    fn to_path_buf(&self) -> PathBuf {
        let mut path = PathBuf::from(&self.0);
        path.set_extension("el");
        path
    }
}

#[derive(Debug, Default)]
struct Options {
    local_only: bool,
}

fn main() {
    let (opts, dir) = parse_options();
    if let Err(err) = resolve_dependencies(&dir, &opts) {
        eprintln!("{}", err);
    }
}

fn parse_options() -> (Options, String) {
    let mut opts = Options::default();
    let mut dir = ".".to_string();
    let mut args = env::args().skip(1);
    if let Some(arg) = args.next() {
        if arg == "-l" {
            opts.local_only = true;
            if let Some(arg) = args.next() {
                dir = arg;
            }
        } else {
            dir = arg;
        }
    }
    (opts, dir)
}

fn resolve_dependencies<P>(dir: P, opts: &Options) -> Result<(), Box<std::error::Error>>
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
    let mut tw = TabWriter::new(std::io::stdout());
    let depgraph = gather_dependencies(&elisps)?;
    for elisp in elisps {
        let mut deps = vec![];
        for d in depgraph.dependencies_of(&elisp)? {
            let file = d?;
            if file == &elisp {
                continue;
            }
            let mut path = PathBuf::from(dir.as_ref());
            path.push(file);
            if let Some(el) = file.to_str() {
                if !opts.local_only || path.is_file() {
                    deps.push(el);
                }
            }
        }
        if let Some(name) = elisp.file_name().and_then(|s| s.to_str()) {
            write!(&mut tw, "\"{}c\"\t[", name)?;
            for (i, dep) in deps.iter().enumerate() {
                if i > 0 {
                    write!(&mut tw, ",")?;
                }
                write!(&mut tw, "\"{}c\"", dep)?;
            }
            writeln!(&mut tw, "]")?;
        }
    }
    tw.flush()?;
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

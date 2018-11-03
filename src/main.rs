#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate tabwriter;

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use regex::Regex;
use tabwriter::TabWriter;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
struct Feature {
    name: String,
    path: Option<PathBuf>,
}

impl Feature {
    fn new(name: &str, path: Option<PathBuf>) -> Self {
        Feature {
            name: name.to_string(),
            path: path,
        }
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn path_buf(&self) -> &Option<PathBuf> {
        &self.path
    }
}

struct DepResolver {
    cache: HashMap<String, Vec<Feature>>,
}

impl DepResolver {
    fn new() -> Self {
        DepResolver {
            cache: HashMap::new(),
        }
    }

    fn depend(&mut self, feat: &Feature, dependencies: Vec<Feature>) {
        self.cache.insert(feat.name().to_string(), dependencies);
    }

    fn dependencies(&self, feat: &Feature) -> DepIterator {
        let deps = self.cache.get(feat.name());
        DepIterator {
            index: 0,
            deps: deps,
        }
    }
}

struct DepIterator<'a> {
    index: usize,
    deps: Option<&'a Vec<Feature>>,
}

impl<'a> Iterator for DepIterator<'a> {
    type Item = &'a Feature;

    fn next(&mut self) -> Option<Self::Item> {
        self.deps.and_then(|v| {
            if self.index < v.len() {
                self.index += 1;
                Some(&v[self.index - 1])
            } else {
                None
            }
        })
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
    let mut features = vec![];
    for entry in dir.as_ref().read_dir()? {
        let path = entry?.path();
        if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
            if ext == "el" {
                features.push(Feature::new(
                    &path
                        .with_extension("")
                        .file_name()
                        .unwrap()
                        .to_string_lossy(),
                    Some(path.clone()),
                ));
            }
        }
    }
    let depgraph = gather_dependencies(&features)?;
    show_dependencies(&dir, &features, &depgraph, opts.local_only)
}

fn show_dependencies<P>(
    dir: &P,
    features: &Vec<Feature>,
    resolver: &DepResolver,
    local_only: bool,
) -> Result<(), Box<std::error::Error>>
where
    P: AsRef<Path>,
{
    let mut tw = TabWriter::new(std::io::stdout());
    for feature in features {
        let mut deps = vec![];
        for dep in resolver.dependencies(&feature) {
            if dep == feature {
                continue;
            }
            if let Some(path_buf) = dep.path_buf() {
                let mut path = PathBuf::from(dir.as_ref());
                path.push(&path_buf);
                if !local_only || path.is_file() {
                    deps.push(dep.name().to_string());
                }
            }
        }
        write!(&mut tw, "\"{}.elc\"\t[", feature.name())?;
        for (i, dep) in deps.iter().enumerate() {
            if i > 0 {
                write!(&mut tw, ",")?;
            }
            write!(&mut tw, "\"{}.elc\"", dep)?;
        }
        writeln!(&mut tw, "]")?;
    }
    tw.flush()?;
    Ok(())
}

fn gather_dependencies(features: &Vec<Feature>) -> Result<DepResolver, Box<std::error::Error>> {
    let mut depgraph = DepResolver::new();
    for feature in features {
        if let Some(path_buf) = feature.path_buf() {
            match extract_requires(&path_buf) {
                Ok(v) => {
                    depgraph.depend(feature, v);
                }
                Err(err) => eprintln!("{}", err),
            }
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
    let file = File::open(&path)?;
    let reader = BufReader::new(file);
    let mut v = vec![];
    for r in reader.lines() {
        let l = r?;
        if let Some(f) = RE.captures(&l).and_then(|c| c.get(1)) {
            let mut req_path = PathBuf::from(path.as_ref().parent().unwrap());
            req_path.push(f.as_str());
            req_path.set_extension("el");
            if req_path.exists() {
                req_path.set_extension("elc");
                v.push(Feature::new(&f.as_str().to_string(), Some(req_path)));
            } else {
                v.push(Feature::new(&f.as_str().to_string(), None));
            }
        }
    }
    Ok(v)
}

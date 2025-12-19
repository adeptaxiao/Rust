use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
};

const STORAGE_FILE: &str = "snippets.json";

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    name: Option<String>,

    #[arg(long)]
    read: Option<String>,

    #[arg(long)]
    delete: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct SnippetStore {
    snippets: BTreeMap<String, String>,
}

impl SnippetStore {
    fn load() -> Self {
        fs::read_to_string(STORAGE_FILE)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        let data = serde_json::to_string_pretty(&self).unwrap();
        fs::write(STORAGE_FILE, data).unwrap();
    }
}

fn main() {
    let args = Cli::parse();
    let mut store = SnippetStore::load();

    if let Some(name) = args.name {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input).unwrap();
        store.snippets.insert(name, input);
        store.save();
        return;
    }

    if let Some(name) = args.read {
        if let Some(snippet) = store.snippets.get(&name) {
            println!("{}", snippet);
        }
        return;
    }

    if let Some(name) = args.delete {
        store.snippets.remove(&name);
        store.save();
    }
}

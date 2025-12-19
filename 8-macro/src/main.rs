//! Main example using btreemap! macro.

use std::collections::BTreeMap;
use crate::btreemap;

fn main() {
    let map: BTreeMap<_, _> = btreemap! {
        "apple" => 3,
        "banana" => 2,
        "orange" => 5,
    };
    for (k, v) in &map {
        println!("{k}: {v}");
    }
}

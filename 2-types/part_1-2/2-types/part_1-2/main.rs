mod part_1;
mod part_2;

fn main() {
    // Part 1 — typestate
    let post = part_1::Post::<part_1::New>::new("Hello Rust!");
    let post = post.publish();
    let post = post.allow();
    let _post = post.delete();
    println!("Part 1 executed successfully.");

    // Part 2 — JSON → TOML
    let json = r#"
    {
        "id": 42,
        "method": "GET",
        "path": "/api/test",
        "headers": [
            { "key": "Content-Type", "value": "application/json" }
        ],
        "body": null
    }
    "#;
    let toml = part_2::convert_json_to_toml(json);
    println!("Part 2 JSON → TOML:\n{}", toml);
}

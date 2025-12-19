use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: HttpMethod,
    pub path: String,
    pub headers: Vec<Header>,
    pub body: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Header {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
}

pub fn convert_json_to_toml(json: &str) -> String {
    let request: Request = serde_json::from_str(json).unwrap();
    toml::to_string(&request).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_to_toml() {
        let json = r#"
        {
            "id": 1,
            "method": "GET",
            "path": "/api/data",
            "headers": [
                { "key": "Accept", "value": "application/json" }
            ],
            "body": null
        }
        "#;

        let toml = convert_json_to_toml(json);
        assert!(toml.contains("GET"));
        assert!(toml.contains("/api/data"));
        assert!(toml.contains("Accept"));
    }
}

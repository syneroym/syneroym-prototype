// TODO: Remove later, just testing correct scaffolding
use serde_json::json;

pub fn intro(name: &str) -> String {
    json!({
        "name": name
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = intro("hello");
        println!("{}", result);
        assert_eq!(result.len(), 16);
    }
}

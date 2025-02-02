use serde_json::{Map, Value};

#[derive(Debug, Default)]
pub struct GMCPStore {
    data: Value,
}

impl GMCPStore {
    /// Create a new (empty) GMCP store.
    pub fn new() -> Self {
        Self {
            data: Value::Object(Map::new()),
        }
    }

    /// Update the GMCP store with a new message.
    ///
    /// `package` is a dot‑separated string (e.g. "room.info" or "char.vitals").
    /// `value` is the JSON value associated with the package.
    pub fn update(&mut self, package: &str, value: Value) {
        let parts: Vec<&str> = package.split('.').collect();
        let mut current = self.data.as_object_mut().expect("GMCPStore data should be an object");
    
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // Clone value only when needed
                current.insert((*part).to_string(), value.clone());
            } else {
                if !current.contains_key(*part) {
                    current.insert((*part).to_string(), Value::Object(Map::new()));
                }
                current = current.get_mut(*part)
                    .and_then(|v| v.as_object_mut())
                    .expect("Expected object in GMCPStore");
            }
        }
    }
    

    /// Retrieve a value from the GMCP store by a dot‑separated key path.
    ///
    /// For example, calling `get("room.info.exits")` returns the corresponding value if present.
    pub fn get(&self, key: &str) -> Option<&Value> {
        let mut current = &self.data;
        for part in key.split('.') {
            if let Some(obj) = current.as_object() {
                if let Some(next) = obj.get(part) {
                    current = next;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        Some(current)
    }
}

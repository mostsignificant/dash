use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub env: HashMap<String, String>,
    pub steps: Vec<Step>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Step {
    pub name: Option<String>,
    pub env: Option<HashMap<String, String>>,
    #[serde(flatten)]
    pub r#type: StepType,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepType {
    Read(ConnectionType),
    Write(ConnectionType),
    Run(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionType {
    File(FileConfig),
    #[serde(alias = "https")]
    Http(HttpConfig),
    Postgresql(PostgresqlConfig),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileConfig {
    pub location: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostgresqlConfig {
    pub connection: String,
    pub query: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HttpConfig {
    pub url: String,
    pub method: Option<HttpMethod>,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

use minijinja::value::{StructObject, Value};
use minijinja::{context, Environment};

struct VarsContext {
    env: HashMap<String, String>,
}

impl VarsContext {
    fn new() -> Self {
        let mut env_vars = HashMap::new();
        for (key, value) in std::env::vars() {
            env_vars.insert(key, value);
        }

        Self { env: env_vars }
    }
}

impl StructObject for VarsContext {
    fn get_field(&self, field: &str) -> Option<Value> {
        match field {
            "env" => Some(Value::from_iter(self.env.clone().into_iter())),
            _ => None,
        }
    }
}

pub fn read_config(file_path: String) -> Config {
    let content = std::fs::read_to_string(file_path).expect("Unable to read config file");

    let re = Regex::new(r"\$\{\{(.*)\}\}").unwrap();
    let content = re.replace_all(&content, "{{$1}}");

    let env = Environment::new();
    let template = env.template_from_str(&content).unwrap();
    let context = Value::from_struct_object(VarsContext::new());
    let resolved = template.render(context).unwrap();

    serde_yaml::from_str(&resolved).expect("config file was not well-formatted")
}

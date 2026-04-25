use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureExtractionResponse {
    pub root_name: String,
    pub categories: Vec<CategoryResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryResponse {
    pub name: String,
    pub description: Option<String>,
    pub features: Vec<String>,
    pub subcategories: Vec<SubcategoryResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubcategoryResponse {
    pub name: String,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonResponse {
    pub directories: Vec<String>,
    pub files: Vec<FileResponse>,
    pub entry_point: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResponse {
    pub path: String,
    pub purpose: String,
    pub component: String,
    pub units: Option<Vec<UnitResponse>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitResponse {
    pub name: String,
    pub kind: String,
    pub signature: Option<String>,
    pub docstring: Option<String>,
    pub features: Vec<String>,
}

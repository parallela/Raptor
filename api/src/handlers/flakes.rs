use axum::{
    extract::{Extension, Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{error::{AppError, AppResult}, models::{AppState, Claims}};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Flake {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub docker_image: String,
    pub startup_command: String,
    pub config_files: serde_json::Value,
    pub startup_detection: Option<String>,
    pub install_script: Option<String>,
    pub install_container: Option<String>,
    pub install_entrypoint: Option<String>,
    pub features: serde_json::Value,
    pub file_denylist: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct FlakeVariable {
    pub id: Uuid,
    pub flake_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub env_variable: String,
    pub default_value: Option<String>,
    pub rules: Option<String>,
    pub user_viewable: bool,
    pub user_editable: bool,
    pub sort_order: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlakeWithVariables {
    #[serde(flatten)]
    pub flake: Flake,
    pub variables: Vec<FlakeVariable>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFlakeRequest {
    pub name: String,
    pub slug: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub docker_image: String,
    pub startup_command: String,
    #[serde(default)]
    pub config_files: serde_json::Value,
    pub startup_detection: Option<String>,
    pub install_script: Option<String>,
    #[serde(default)]
    pub variables: Vec<CreateVariableRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateVariableRequest {
    pub name: String,
    pub description: Option<String>,
    pub env_variable: String,
    #[serde(default)]
    pub default_value: String,
    #[serde(default = "default_rules")]
    pub rules: String,
    #[serde(default = "default_true")]
    pub user_viewable: bool,
    #[serde(default = "default_true")]
    pub user_editable: bool,
}

fn default_rules() -> String {
    "nullable|string".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFlakeRequest {
    pub egg_json: serde_json::Value,
}

/// List all flakes
pub async fn list_flakes(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> AppResult<Json<Vec<Flake>>> {
    if claims.sub == Uuid::nil() {
        return Err(AppError::Unauthorized);
    }

    let flakes: Vec<Flake> = sqlx::query_as("SELECT * FROM flakes ORDER BY name")
        .fetch_all(&state.db)
        .await?;

    Ok(Json(flakes))
}

/// Get a flake with its variables
pub async fn get_flake(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<FlakeWithVariables>> {
    if claims.sub == Uuid::nil() {
        return Err(AppError::Unauthorized);
    }

    let flake: Flake = sqlx::query_as("SELECT * FROM flakes WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let variables: Vec<FlakeVariable> = sqlx::query_as(
        "SELECT * FROM flake_variables WHERE flake_id = $1 ORDER BY sort_order"
    )
        .bind(id)
        .fetch_all(&state.db)
        .await?;

    Ok(Json(FlakeWithVariables { flake, variables }))
}

/// Create a new flake (admin only)
pub async fn create_flake(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateFlakeRequest>,
) -> AppResult<Json<FlakeWithVariables>> {
    if !claims.has_permission("flakes.create") && !claims.is_admin() {
        return Err(AppError::Unauthorized);
    }

    let flake_id = Uuid::new_v4();

    let flake: Flake = sqlx::query_as(
        r#"INSERT INTO flakes (id, name, slug, author, description, docker_image, startup_command, config_files, startup_detection, install_script)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING *"#
    )
        .bind(flake_id)
        .bind(&req.name)
        .bind(&req.slug)
        .bind(&req.author)
        .bind(&req.description)
        .bind(&req.docker_image)
        .bind(&req.startup_command)
        .bind(&req.config_files)
        .bind(&req.startup_detection)
        .bind(&req.install_script)
        .fetch_one(&state.db)
        .await?;

    let mut variables = Vec::new();
    for (idx, var) in req.variables.iter().enumerate() {
        let v: FlakeVariable = sqlx::query_as(
            r#"INSERT INTO flake_variables (id, flake_id, name, description, env_variable, default_value, rules, user_viewable, user_editable, sort_order)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING *"#
        )
            .bind(Uuid::new_v4())
            .bind(flake_id)
            .bind(&var.name)
            .bind(&var.description)
            .bind(&var.env_variable)
            .bind(&var.default_value)
            .bind(&var.rules)
            .bind(var.user_viewable)
            .bind(var.user_editable)
            .bind(idx as i32)
            .fetch_one(&state.db)
            .await?;
        variables.push(v);
    }

    Ok(Json(FlakeWithVariables { flake, variables }))
}

/// Delete a flake (admin only)
pub async fn delete_flake(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    if !claims.has_permission("flakes.delete") && !claims.is_admin() {
        return Err(AppError::Unauthorized);
    }

    sqlx::query("DELETE FROM flakes WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "message": "Flake deleted" })))
}

/// Import a Pterodactyl egg as a flake (admin only)
pub async fn import_flake(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ImportFlakeRequest>,
) -> AppResult<Json<FlakeWithVariables>> {
    if !claims.has_permission("flakes.create") && !claims.is_admin() {
        return Err(AppError::Unauthorized);
    }

    let egg = &req.egg_json;

    let name = egg["name"].as_str().unwrap_or("Imported").to_string();
    let base_slug = egg["slug"].as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| name.to_lowercase().replace(' ', "-").replace("(", "").replace(")", ""));

    // Check if slug exists and generate a unique one if needed
    let mut slug = base_slug.clone();
    let mut counter = 1;
    loop {
        let existing: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM flakes WHERE slug = $1")
            .bind(&slug)
            .fetch_optional(&state.db)
            .await?;
        if existing.is_none() {
            break;
        }
        slug = format!("{}_{}", base_slug, counter);
        counter += 1;
    }

    let author = egg["author"].as_str().map(|s| s.to_string());
    let description = egg["description"].as_str().map(|s| s.to_string());
    let startup_command = egg["startup"].as_str()
        .or_else(|| egg["startupCommand"].as_str())
        .unwrap_or("").to_string();

    // Use provided dockerImage or default to our artifact
    let docker_image = egg["dockerImage"].as_str()
        .or_else(|| egg["docker_image"].as_str())
        .unwrap_or("artifacts.lstan.eu/java:21").to_string();

    let mut config_files = serde_json::json!({});
    if let Some(config) = egg["config"]["files"].as_str() {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(config) {
            config_files = parsed;
        }
    } else if let Some(config) = egg["configFiles"].as_object() {
        config_files = serde_json::json!(config);
    }

    let startup_detection = egg["config"]["startup"].as_str()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| v["done"].as_str().map(|s| s.to_string()))
        .or_else(|| egg["startupDetection"].as_str().map(|s| s.to_string()));

    let mut install_script = egg["scripts"]["installation"]["script"].as_str()
        .or_else(|| egg["installScript"].as_str())
        .map(|s| s.to_string());
    if let Some(ref mut script) = install_script {
        if !script.contains("eula=true") {
            script.push_str("\necho 'eula=true' > eula.txt\n");
        }
    }

    // Use a transaction to ensure atomicity
    let mut tx = state.db.begin().await?;

    let flake_id = Uuid::new_v4();

    let flake: Flake = sqlx::query_as(
        r#"INSERT INTO flakes (id, name, slug, author, description, docker_image, startup_command, config_files, startup_detection, install_script)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING *"#
    )
        .bind(flake_id)
        .bind(&name)
        .bind(&slug)
        .bind(&author)
        .bind(&description)
        .bind(&docker_image)
        .bind(&startup_command)
        .bind(&config_files)
        .bind(&startup_detection)
        .bind(&install_script)
        .fetch_one(&mut *tx)
        .await?;

    let mut variables = Vec::new();
    let vars_array = egg["variables"].as_array();
    if let Some(vars) = vars_array {
        // Track env variables to avoid duplicates within the same import
        let mut seen_env_vars = std::collections::HashSet::new();

        for (idx, var) in vars.iter().enumerate() {
            let env_var = var["env_variable"].as_str()
                .or_else(|| var["envVariable"].as_str())
                .unwrap_or("VAR").to_string();

            // Skip duplicate env variables
            if seen_env_vars.contains(&env_var) {
                continue;
            }
            seen_env_vars.insert(env_var.clone());

            let v: FlakeVariable = sqlx::query_as(
                r#"INSERT INTO flake_variables (id, flake_id, name, description, env_variable, default_value, rules, user_viewable, user_editable, sort_order)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                RETURNING *"#
            )
                .bind(Uuid::new_v4())
                .bind(flake_id)
                .bind(var["name"].as_str().unwrap_or("Variable"))
                .bind(var["description"].as_str())
                .bind(&env_var)
                .bind(var["default_value"].as_str().or_else(|| var["defaultValue"].as_str()).unwrap_or(""))
                .bind(var["rules"].as_str().unwrap_or("nullable|string"))
                .bind(var["user_viewable"].as_bool().or_else(|| var["userViewable"].as_bool()).unwrap_or(true))
                .bind(var["user_editable"].as_bool().or_else(|| var["userEditable"].as_bool()).unwrap_or(true))
                .bind(idx as i32)
                .fetch_one(&mut *tx)
                .await?;
            variables.push(v);
        }
    }

    // Commit the transaction
    tx.commit().await?;

    Ok(Json(FlakeWithVariables { flake, variables }))
}

/// Export a flake as Pterodactyl egg format
pub async fn export_flake(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    if claims.sub == Uuid::nil() {
        return Err(AppError::Unauthorized);
    }

    let flake: Flake = sqlx::query_as("SELECT * FROM flakes WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let variables: Vec<FlakeVariable> = sqlx::query_as(
        "SELECT * FROM flake_variables WHERE flake_id = $1 ORDER BY sort_order"
    )
        .bind(id)
        .fetch_all(&state.db)
        .await?;

    let egg = serde_json::json!({
        "_comment": "Exported from Raptor Panel",
        "meta": { "version": "RAPTOR_v1" },
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "name": flake.name,
        "author": flake.author,
        "description": flake.description,
        "docker_images": { "Java 21": flake.docker_image },
        "startup": flake.startup_command,
        "config": {
            "files": serde_json::to_string(&flake.config_files).unwrap_or_default()
        },
        "variables": variables.iter().map(|v| serde_json::json!({
            "name": v.name,
            "description": v.description,
            "env_variable": v.env_variable,
            "default_value": v.default_value,
            "user_viewable": v.user_viewable,
            "user_editable": v.user_editable,
            "rules": v.rules
        })).collect::<Vec<_>>()
    });

    Ok(Json(egg))
}

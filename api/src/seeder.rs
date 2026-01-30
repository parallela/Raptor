use bcrypt::hash;
use rand::Rng;
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Config;

/// Generate a random password
fn generate_password(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

pub async fn run(pool: &PgPool, config: &Config) -> anyhow::Result<()> {
    if !was_executed(pool, "permissions").await? {
        seed_permissions(pool).await?;
        mark_executed(pool, "permissions").await?;
    }

    if !was_executed(pool, "roles").await? {
        seed_roles(pool).await?;
        mark_executed(pool, "roles").await?;
    }

    if !was_executed(pool, "role_permissions").await? {
        seed_role_permissions(pool).await?;
        mark_executed(pool, "role_permissions").await?;
    }

    if !was_executed(pool, "admin_user").await? {
        seed_admin_user(pool, config).await?;
        mark_executed(pool, "admin_user").await?;
    }

    if !was_executed(pool, "database_server_passwords").await? {
        seed_database_server_passwords(pool).await?;
        mark_executed(pool, "database_server_passwords").await?;
    }

    Ok(())
}

async fn was_executed(pool: &PgPool, name: &str) -> anyhow::Result<bool> {
    let result: Option<(String,)> = sqlx::query_as("SELECT name FROM _seeders WHERE name = $1")
        .bind(name)
        .fetch_optional(pool)
        .await?;
    Ok(result.is_some())
}

async fn mark_executed(pool: &PgPool, name: &str) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO _seeders (name) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(name)
        .execute(pool)
        .await?;
    tracing::info!("seeder.{} executed", name);
    Ok(())
}

async fn seed_permissions(pool: &PgPool) -> anyhow::Result<()> {
    let permissions = [
        ("*", "Full access to all resources"),
        ("admin.access", "Access to admin panel"),
        ("users.view", "View users"),
        ("users.create", "Create users"),
        ("users.update", "Update users"),
        ("users.delete", "Delete users"),
        ("roles.view", "View roles"),
        ("roles.create", "Create roles"),
        ("roles.update", "Update roles"),
        ("roles.delete", "Delete roles"),
        ("daemons.view", "View daemons"),
        ("daemons.create", "Create daemons"),
        ("daemons.update", "Update daemons"),
        ("daemons.delete", "Delete daemons"),
        ("containers.view_own", "View own containers"),
        ("containers.view_all", "View all containers"),
        ("containers.create", "Create containers"),
        ("containers.update", "Update containers"),
        ("containers.delete", "Delete containers"),
        ("containers.manage", "Manage containers (start/stop/restart)"),
        ("containers.manage_own", "Manage own containers"),
        ("allocations.view", "View allocations"),
        ("allocations.create", "Create allocations"),
        ("allocations.delete", "Delete allocations"),
        ("flakes.view", "View flakes (server templates)"),
        ("flakes.create", "Create flakes"),
        ("flakes.update", "Update flakes"),
        ("flakes.delete", "Delete flakes"),
    ];

    for (name, description) in permissions {
        sqlx::query(
            "INSERT INTO permissions (id, name, description) VALUES ($1, $2, $3) ON CONFLICT (name) DO NOTHING"
        )
        .bind(Uuid::new_v4())
        .bind(name)
        .bind(description)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn seed_roles(pool: &PgPool) -> anyhow::Result<()> {
    let roles = ["admin", "manager", "user"];

    for name in roles {
        sqlx::query(
            "INSERT INTO roles (id, name, permissions, created_at, updated_at) VALUES ($1, $2, '{}', NOW(), NOW()) ON CONFLICT (name) DO NOTHING"
        )
        .bind(Uuid::new_v4())
        .bind(name)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn seed_role_permissions(pool: &PgPool) -> anyhow::Result<()> {
    let admin_perms = vec!["*"];
    let manager_perms = vec![
        "admin.access", "users.view", "users.create", "users.update",
        "daemons.view", "containers.view_all", "containers.create",
        "containers.update", "containers.delete", "containers.manage",
        "allocations.view", "allocations.create",
        "flakes.view", "flakes.create", "flakes.update", "flakes.delete",
    ];
    let user_perms = vec!["containers.view_own", "containers.manage_own"];

    assign_permissions_to_role(pool, "admin", &admin_perms).await?;
    assign_permissions_to_role(pool, "manager", &manager_perms).await?;
    assign_permissions_to_role(pool, "user", &user_perms).await?;

    Ok(())
}

async fn assign_permissions_to_role(pool: &PgPool, role_name: &str, permission_names: &[&str]) -> anyhow::Result<()> {
    let role: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM roles WHERE name = $1")
        .bind(role_name)
        .fetch_optional(pool)
        .await?;

    let role_id = match role {
        Some((id,)) => id,
        None => return Ok(()),
    };

    for perm_name in permission_names {
        let perm: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM permissions WHERE name = $1")
            .bind(*perm_name)
            .fetch_optional(pool)
            .await?;

        if let Some((perm_id,)) = perm {
            sqlx::query(
                "INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2) ON CONFLICT DO NOTHING"
            )
            .bind(role_id)
            .bind(perm_id)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

async fn seed_admin_user(pool: &PgPool, config: &Config) -> anyhow::Result<()> {
    let password = match &config.admin.password {
        Some(pwd) if !pwd.is_empty() => pwd.clone(),
        _ => {
            tracing::warn!("ADMIN_PASSWORD not set, skipping admin user creation");
            return Ok(());
        }
    };

    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM users WHERE username = $1 OR email = $2"
    )
    .bind(&config.admin.username)
    .bind(&config.admin.email)
    .fetch_optional(pool)
    .await?;

    if existing.is_some() {
        return Ok(());
    }

    let admin_role: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM roles WHERE name = 'admin'")
        .fetch_optional(pool)
        .await?;

    let admin_role_id = admin_role
        .map(|(id,)| id)
        .ok_or_else(|| anyhow::anyhow!("Admin role not found"))?;

    let password_hash = hash(&password, config.bcrypt_cost)
        .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;

    sqlx::query(
        r#"
        INSERT INTO users (id, username, email, password_hash, role_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
        "#
    )
    .bind(Uuid::new_v4())
    .bind(&config.admin.username)
    .bind(&config.admin.email)
    .bind(&password_hash)
    .bind(admin_role_id)
    .execute(pool)
    .await?;

    tracing::info!("Admin user '{}' created", config.admin.username);
    Ok(())
}

async fn seed_database_server_passwords(pool: &PgPool) -> anyhow::Result<()> {
    // Update any database servers that still have placeholder passwords
    let servers: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, root_password FROM database_servers WHERE root_password LIKE 'CHANGE_ME%'"
    )
    .fetch_all(pool)
    .await?;

    for (id, _) in servers {
        let new_password = generate_password(32);
        sqlx::query(
            "UPDATE database_servers SET root_password = $1, updated_at = NOW() WHERE id = $2"
        )
        .bind(&new_password)
        .bind(id)
        .execute(pool)
        .await?;
        tracing::info!("Generated new root password for database server {}", id);
    }

    Ok(())
}


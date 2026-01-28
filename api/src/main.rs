mod config;
mod email;
mod error;
mod handlers;
mod middleware;
mod models;
mod seeder;

use axum::{
    routing::{get, post, delete, patch},
    middleware as axum_middleware,
    Router,
};
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::middleware::{require_permission, require_admin, require_manager};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("../migrations").run(&pool).await?;

    seeder::run(&pool, &config).await?;

    let app_state = models::AppState {
        db: pool,
        config: config.clone(),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let public_routes = Router::new()
        .route("/", get(|| async { "Raptor API" }))
        .route("/health", get(|| async { "OK" }))
        .route("/auth/login", post(handlers::auth::login))
        .route("/auth/register", post(handlers::auth::register))
        .route("/auth/forgot-password", post(handlers::auth::forgot_password))
        .route("/auth/reset-password", post(handlers::auth::reset_password))
        .route("/auth/accept-invite", post(handlers::users::accept_invite))
        .route("/ws/daemons/stats", get(handlers::daemons::ws_daemon_stats))
        // WebSocket routes - auth via query param
        .route("/ws/containers/:id/logs", get(handlers::ws::container_logs))
        .route("/ws/containers/:id/stats", get(handlers::ws::container_stats));

    let user_routes = Router::new()
        .route("/users/me", get(handlers::users::get_me))
        .route("/containers", get(handlers::containers::list_containers))
        .route("/containers/:id", get(handlers::containers::get_container))
        .route("/containers/:id", patch(handlers::containers::update_container))
        .route("/containers/:id/ports", get(handlers::containers::get_container_ports))
        .route("/containers/:id/stats", get(handlers::containers::get_container_stats))
        .route("/containers/:id/allocation", post(handlers::containers::assign_allocation))
        .route("/containers/:id/allocations", get(handlers::containers::get_container_allocations))
        .route("/containers/:id/allocations/available", get(handlers::containers::get_available_allocations))
        .route("/containers/:id/allocations", post(handlers::containers::add_allocation))
        .route("/containers/:id/allocations/:allocation_id", delete(handlers::containers::remove_allocation))
        .route("/containers/:id/start", post(handlers::containers::start_container))
        .route("/containers/:id/stop", post(handlers::containers::stop_container))
        .route("/containers/:id/restart", post(handlers::containers::restart_container))
        .route("/containers/:id/kill", post(handlers::containers::kill_container))
        .route("/containers/:id/command", post(handlers::containers::send_command))
        .route("/containers/:id/graceful-stop", post(handlers::containers::graceful_stop_container))
        .route("/containers/:id/sftp-password", post(handlers::containers::set_sftp_password))
        .route("/containers/:id/users", get(handlers::containers::list_container_users))
        .route("/containers/:id/users", post(handlers::containers::add_container_user))
        .route("/containers/:id/users/:user_id", delete(handlers::containers::remove_container_user))
        .route("/containers/:id/files", get(handlers::containers::list_files))
        .route("/containers/:id/files/read", get(handlers::containers::read_file))
        .route("/containers/:id/files/write", post(handlers::containers::write_file))
        .route("/containers/:id/files/folder", post(handlers::containers::create_folder))
        .route("/containers/:id/files/delete", delete(handlers::containers::delete_file))
        .route("/daemons", get(handlers::daemons::list_daemons))
        .route("/daemons/:id", get(handlers::daemons::get_daemon))
        .route("/daemons/:id/status", get(handlers::daemons::get_daemon_status))
        .route("/roles", get(handlers::roles::list_roles))
        .route("/roles/:id", get(handlers::roles::get_role))
        // Flakes (server templates)
        .route("/flakes", get(handlers::flakes::list_flakes))
        .route("/flakes/:id", get(handlers::flakes::get_flake))
        .route("/flakes/:id/export", get(handlers::flakes::export_flake));

    let manager_routes = Router::new()
        .route("/users", get(handlers::users::list_users))
        .route("/users/:id", get(handlers::users::get_user))
        .route("/users/:id", patch(handlers::users::update_user))
        .route("/containers", post(handlers::containers::create_container)
            .route_layer(axum_middleware::from_fn(require_permission("containers.create"))))
        .route("/containers/:id", delete(handlers::containers::delete_container)
            .route_layer(axum_middleware::from_fn(require_permission("containers.delete"))))
        .route("/admin/containers", get(handlers::containers::list_all_containers)
            .route_layer(axum_middleware::from_fn(require_permission("containers.view_all"))))
        .route("/daemons/:id/ip-pools", get(handlers::allocations::list_daemon_ip_pools))
        .route("/allocations", get(handlers::allocations::list_allocations))
        .route("/allocations/all", get(handlers::allocations::list_all_allocations))
        .route("/allocations", post(handlers::allocations::create_allocation)
            .route_layer(axum_middleware::from_fn(require_permission("allocations.create"))))
        .route("/ip-pools", get(handlers::allocations::list_ip_pools))
        .route("/ip-pools", post(handlers::allocations::create_ip_pool)
            .route_layer(axum_middleware::from_fn(require_permission("allocations.create"))))
        .route("/ip-pools/:id", delete(handlers::allocations::delete_ip_pool)
            .route_layer(axum_middleware::from_fn(require_permission("allocations.delete"))))
        .route("/container-allocations", post(handlers::allocations::create_container_allocation)
            .route_layer(axum_middleware::from_fn(require_permission("allocations.create"))))
        .route("/container-allocations/:id", delete(handlers::allocations::delete_container_allocation)
            .route_layer(axum_middleware::from_fn(require_permission("allocations.delete"))))
        // Flakes management (admin only)
        .route("/flakes", post(handlers::flakes::create_flake)
            .route_layer(axum_middleware::from_fn(require_permission("flakes.create"))))
        .route("/flakes/import", post(handlers::flakes::import_flake)
            .route_layer(axum_middleware::from_fn(require_permission("flakes.create"))))
        .route("/flakes/:id", delete(handlers::flakes::delete_flake)
            .route_layer(axum_middleware::from_fn(require_permission("flakes.delete"))))
        .layer(axum_middleware::from_fn(require_manager));

    let admin_routes = Router::new()
        .route("/users/search", get(handlers::users::search_users))
        .route("/users/invite", post(handlers::users::invite_user)
            .route_layer(axum_middleware::from_fn(require_permission("users.create"))))
        .route("/users/:id", delete(handlers::users::delete_user)
            .route_layer(axum_middleware::from_fn(require_permission("users.delete"))))
        .route("/daemons", post(handlers::daemons::create_daemon)
            .route_layer(axum_middleware::from_fn(require_permission("daemons.create"))))
        .route("/daemons/:id", patch(handlers::daemons::update_daemon)
            .route_layer(axum_middleware::from_fn(require_permission("daemons.update"))))
        .route("/daemons/:id", delete(handlers::daemons::delete_daemon)
            .route_layer(axum_middleware::from_fn(require_permission("daemons.delete"))))
        .route("/roles", post(handlers::roles::create_role)
            .route_layer(axum_middleware::from_fn(require_permission("roles.create"))))
        .route("/roles/:id", patch(handlers::roles::update_role)
            .route_layer(axum_middleware::from_fn(require_permission("roles.update"))))
        .route("/roles/:id", delete(handlers::roles::delete_role)
            .route_layer(axum_middleware::from_fn(require_permission("roles.delete"))))
        .layer(axum_middleware::from_fn(require_admin));

    let protected_routes = Router::new()
        .merge(user_routes)
        .merge(manager_routes)
        .merge(admin_routes)
        .layer(axum_middleware::from_fn_with_state(
            app_state.clone(),
            middleware::auth,
        ));

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&config.api_addr).await?;
    tracing::info!("listening on {}", config.api_addr);
    axum::serve(listener, app).await?;

    Ok(())
}

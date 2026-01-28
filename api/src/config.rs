
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expiry_days: i64,
    pub api_addr: String,
    pub app_url: String,
    pub bcrypt_cost: u32,
    pub smtp: Option<SmtpConfig>,
    pub admin: AdminConfig,
}

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
}

#[derive(Debug, Clone)]
pub struct AdminConfig {
    pub username: String,
    pub email: String,
    pub password: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        let app_key = std::env::var("APP_KEY")
            .expect("APP_KEY must be set - generate with: openssl rand -base64 32");

        let smtp = Self::load_smtp_config();

        Self {
            database_url: std::env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set"),
            jwt_secret: std::env::var("JWT_SECRET").unwrap_or(app_key),
            jwt_expiry_days: std::env::var("JWT_EXPIRY_DAYS")
                .unwrap_or_else(|_| "7".into())
                .parse()
                .unwrap_or(7),
            api_addr: std::env::var("API_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into()),
            app_url: std::env::var("APP_URL").unwrap_or_else(|_| "http://localhost:5173".into()),
            bcrypt_cost: std::env::var("BCRYPT_COST")
                .unwrap_or_else(|_| "12".into())
                .parse()
                .unwrap_or(12),
            smtp,
            admin: AdminConfig {
                username: std::env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".into()),
                email: std::env::var("ADMIN_EMAIL").unwrap_or_else(|_| "admin@localhost".into()),
                password: std::env::var("ADMIN_PASSWORD").ok(),
            },
        }
    }

    fn load_smtp_config() -> Option<SmtpConfig> {
        let host = std::env::var("SMTP_HOST").ok()?;
        let username = std::env::var("SMTP_USERNAME").ok()?;
        let password = std::env::var("SMTP_PASSWORD").ok()?;

        Some(SmtpConfig {
            host,
            port: std::env::var("SMTP_PORT")
                .unwrap_or_else(|_| "587".into())
                .parse()
                .unwrap_or(587),
            username,
            password,
            from_email: std::env::var("SMTP_FROM_EMAIL").unwrap_or_else(|_| "noreply@localhost".into()),
            from_name: std::env::var("SMTP_FROM_NAME").unwrap_or_else(|_| "Raptor".into()),
        })
    }
}

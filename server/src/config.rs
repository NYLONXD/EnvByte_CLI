use std::{env, net::SocketAddr, str::FromStr};

#[derive(Clone, Debug)]
pub struct Config {
    pub environment: String,
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_issuer: String,
    pub access_token_ttl_seconds: i64,
    pub refresh_token_ttl_days: i64,
    pub invite_ttl_hours: i64,
    pub email_verification_ttl_hours: i64,
    pub password_reset_ttl_minutes: i64,
    pub database_max_connections: u32,
    pub public_cli_url: String,
    pub rate_limit_per_minute: u32,
    pub auth_rate_limit_per_minute: u32,
    pub trusted_proxy_hops: usize,
    pub email: EmailConfig,
}

#[derive(Clone, Debug)]
pub enum EmailConfig {
    Log,
    Smtp {
        host: String,
        port: u16,
        username: String,
        password: String,
        from: String,
    },
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let environment = env::var("GREENBYTE_ENV").unwrap_or_else(|_| "development".to_string());
        let database_url = required("DATABASE_URL")?;
        let jwt_secret = required("GREENBYTE_JWT_SECRET")?;
        if jwt_secret.len() < 32 {
            return Err("GREENBYTE_JWT_SECRET must be at least 32 characters".to_string());
        }
        let email = match env::var("GREENBYTE_EMAIL_MODE")
            .unwrap_or_else(|_| "log".to_string())
            .as_str()
        {
            "log" => EmailConfig::Log,
            "smtp" => EmailConfig::Smtp {
                host: required("SMTP_HOST")?,
                port: parse("SMTP_PORT", 587)?,
                username: required("SMTP_USERNAME")?,
                password: required("SMTP_PASSWORD")?,
                from: required("SMTP_FROM")?,
            },
            value => return Err(format!("Unsupported GREENBYTE_EMAIL_MODE: {value}")),
        };
        if environment == "production" && matches!(email, EmailConfig::Log) {
            return Err("GREENBYTE_EMAIL_MODE=log is forbidden in production".to_string());
        }
        Ok(Self {
            environment,
            bind_addr: parse("GREENBYTE_BIND", SocketAddr::from(([0, 0, 0, 0], 3030)))?,
            database_url,
            jwt_secret,
            jwt_issuer: env::var("GREENBYTE_JWT_ISSUER")
                .unwrap_or_else(|_| "greenbyte".to_string()),
            access_token_ttl_seconds: parse("GREENBYTE_ACCESS_TTL_SECONDS", 3600)?,
            refresh_token_ttl_days: parse("GREENBYTE_REFRESH_TTL_DAYS", 30)?,
            invite_ttl_hours: parse("GREENBYTE_INVITE_TTL_HOURS", 24)?,
            email_verification_ttl_hours: parse("GREENBYTE_EMAIL_VERIFICATION_TTL_HOURS", 2)?,
            password_reset_ttl_minutes: parse("GREENBYTE_PASSWORD_RESET_TTL_MINUTES", 30)?,
            database_max_connections: parse("DATABASE_MAX_CONNECTIONS", 20)?,
            public_cli_url: env::var("GREENBYTE_PUBLIC_CLI_URL")
                .unwrap_or_else(|_| "https://github.com/greenbyte/greenbyte".to_string()),
            rate_limit_per_minute: parse("GREENBYTE_RATE_LIMIT_PER_MINUTE", 300)?,
            auth_rate_limit_per_minute: parse("GREENBYTE_AUTH_RATE_LIMIT_PER_MINUTE", 20)?,
            trusted_proxy_hops: parse("GREENBYTE_TRUSTED_PROXY_HOPS", 0)?,
            email,
        })
    }
}

fn required(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("Missing required environment variable {name}"))
}

fn parse<T>(name: &str, default: T) -> Result<T, String>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(name) {
        Ok(value) => value.parse().map_err(|e| format!("Invalid {name}: {e}")),
        Err(_) => Ok(default),
    }
}

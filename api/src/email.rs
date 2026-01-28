use lettre::{
    message::header::ContentType,
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

use crate::config::SmtpConfig;

pub struct EmailService {
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    from_email: String,
    from_name: String,
    app_url: String,
}

impl EmailService {
    pub fn new(config: &SmtpConfig, app_url: &str) -> Result<Self, lettre::transport::smtp::Error> {
        let creds = Credentials::new(config.username.clone(), config.password.clone());

        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)?
            .port(config.port)
            .credentials(creds)
            .build();

        Ok(Self {
            mailer,
            from_email: config.from_email.clone(),
            from_name: config.from_name.clone(),
            app_url: app_url.to_string(),
        })
    }

    fn load_template(name: &str) -> String {
        let path = format!("templates/{}.html", name);
        std::fs::read_to_string(&path).unwrap_or_else(|_| {
            tracing::warn!("Template {} not found, using fallback", path);
            r#"<!DOCTYPE html><html><body style="font-family: sans-serif; background: #1e293b; color: #e2e8f0; padding: 40px;">
                <div style="max-width: 600px; margin: 0 auto;"><h2>{{SUBJECT}}</h2><p>{{CONTENT}}</p></div>
            </body></html>"#.to_string()
        })
    }

    pub async fn send_password_reset_email(
        &self,
        to_email: &str,
        to_name: &str,
        reset_token: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let reset_link = format!("{}/reset-password?token={}", self.app_url, reset_token);
        
        let html_body = Self::load_template("password_reset")
            .replace("{{APP_URL}}", &self.app_url)
            .replace("{{NAME}}", to_name)
            .replace("{{RESET_LINK}}", &reset_link);

        let email = Message::builder()
            .from(format!("{} <{}>", self.from_name, self.from_email).parse()?)
            .to(format!("{} <{}>", to_name, to_email).parse()?)
            .subject("Reset Your Password - Raptor")
            .header(ContentType::TEXT_HTML)
            .body(html_body)?;

        self.mailer.send(email).await?;
        Ok(())
    }

    pub async fn send_welcome_email(
        &self,
        to_email: &str,
        to_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let html_body = Self::load_template("welcome")
            .replace("{{APP_URL}}", &self.app_url)
            .replace("{{NAME}}", to_name);

        let email = Message::builder()
            .from(format!("{} <{}>", self.from_name, self.from_email).parse()?)
            .to(format!("{} <{}>", to_name, to_email).parse()?)
            .subject("Welcome to Raptor!")
            .header(ContentType::TEXT_HTML)
            .body(html_body)?;

        self.mailer.send(email).await?;
        Ok(())
    }

    pub async fn send_invite_email(
        &self,
        to_email: &str,
        invite_token: &str,
        inviter_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let invite_link = format!("{}/invite?token={}", self.app_url, invite_token);

        let html_body = Self::load_template("invite")
            .replace("{{APP_URL}}", &self.app_url)
            .replace("{{INVITER}}", inviter_name)
            .replace("{{INVITE_LINK}}", &invite_link);

        let email = Message::builder()
            .from(format!("{} <{}>", self.from_name, self.from_email).parse()?)
            .to(to_email.parse()?)
            .subject("You've been invited to Raptor!")
            .header(ContentType::TEXT_HTML)
            .body(html_body)?;

        self.mailer.send(email).await?;
        Ok(())
    }
}

pub fn generate_reset_token() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..64)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

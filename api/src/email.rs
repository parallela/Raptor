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

        let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)?
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

        let paths = [
            format!("templates/{}.html", name),
            format!("/app/templates/{}.html", name),
            format!("./templates/{}.html", name),
        ];

        for path in &paths {
            if let Ok(content) = std::fs::read_to_string(path) {
                return content;
            }
        }

        tracing::warn!("Template templates/{}.html not found in any location, using embedded fallback", name);

        match name {
            "invite" => r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; background: #0f172a; color: #e2e8f0; padding: 20px; margin: 0; }
        .container { max-width: 600px; margin: 0 auto; background: #1e293b; padding: 40px; border-radius: 12px; }
        h2 { color: #0ea5e9; margin-top: 0; }
        p { line-height: 1.6; color: #cbd5e1; }
        .highlight { color: #0ea5e9; font-weight: 600; }
        .btn { display: inline-block; background: #0ea5e9; color: white !important; padding: 14px 28px; text-decoration: none; border-radius: 8px; font-weight: 600; margin: 20px 0; }
        .footer { margin-top: 30px; font-size: 12px; color: #64748b; border-top: 1px solid #334155; padding-top: 20px; }
    </style>
</head>
<body>
    <div class="container">
        <h2>You've Been Invited! 🎉</h2>
        <p>Hello,</p>
        <p><span class="highlight">{{INVITER}}</span> has invited you to join Raptor, a powerful container management platform.</p>
        <p><a href="{{INVITE_LINK}}" class="btn">Accept Invitation</a></p>
        <p>This invitation will expire in 7 days.</p>
        <div class="footer">Raptor - Container Management Panel</div>
    </div>
</body>
</html>"#.to_string(),
            "password_reset" => r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; background: #0f172a; color: #e2e8f0; padding: 20px; margin: 0; }
        .container { max-width: 600px; margin: 0 auto; background: #1e293b; padding: 40px; border-radius: 12px; }
        h2 { color: #0ea5e9; margin-top: 0; }
        p { line-height: 1.6; color: #cbd5e1; }
        .btn { display: inline-block; background: #0ea5e9; color: white !important; padding: 14px 28px; text-decoration: none; border-radius: 8px; font-weight: 600; margin: 20px 0; }
        .footer { margin-top: 30px; font-size: 12px; color: #64748b; border-top: 1px solid #334155; padding-top: 20px; }
    </style>
</head>
<body>
    <div class="container">
        <h2>Reset Your Password</h2>
        <p>Hello {{NAME}},</p>
        <p>We received a request to reset your password for your Raptor account.</p>
        <p><a href="{{RESET_LINK}}" class="btn">Reset Password</a></p>
        <p>If you didn't request this, you can safely ignore this email.</p>
        <div class="footer">Raptor - Container Management Panel</div>
    </div>
</body>
</html>"#.to_string(),
            "welcome" => r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; background: #0f172a; color: #e2e8f0; padding: 20px; margin: 0; }
        .container { max-width: 600px; margin: 0 auto; background: #1e293b; padding: 40px; border-radius: 12px; }
        h2 { color: #0ea5e9; margin-top: 0; }
        p { line-height: 1.6; color: #cbd5e1; }
        .btn { display: inline-block; background: #0ea5e9; color: white !important; padding: 14px 28px; text-decoration: none; border-radius: 8px; font-weight: 600; margin: 20px 0; }
        .footer { margin-top: 30px; font-size: 12px; color: #64748b; border-top: 1px solid #334155; padding-top: 20px; }
    </style>
</head>
<body>
    <div class="container">
        <h2>Welcome to Raptor! 🚀</h2>
        <p>Hello {{NAME}},</p>
        <p>Your account has been successfully created. You can now access the Raptor panel.</p>
        <p><a href="{{APP_URL}}" class="btn">Go to Dashboard</a></p>
        <div class="footer">Raptor - Container Management Panel</div>
    </div>
</body>
</html>"#.to_string(),
            _ => r#"<!DOCTYPE html>
<html>
<body style="font-family: sans-serif; background: #1e293b; color: #e2e8f0; padding: 40px;">
    <div style="max-width: 600px; margin: 0 auto;"><h2>Raptor Notification</h2><p>You have a notification from Raptor.</p></div>
</body>
</html>"#.to_string()
        }
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

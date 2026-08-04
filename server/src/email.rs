use async_trait::async_trait;
use lettre::{
    message::Mailbox, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};

use crate::config::EmailConfig;

#[async_trait]
pub trait EmailSender: Send + Sync {
    async fn send_verification(&self, recipient: &str, token: &str) -> Result<(), String>;

    async fn send_password_reset(&self, recipient: &str, token: &str) -> Result<(), String>;

    async fn send_invitation(
        &self,
        recipient: &str,
        project: &str,
        token: &str,
        cli_url: &str,
    ) -> Result<(), String>;
}

pub fn build_sender(config: &EmailConfig) -> Result<std::sync::Arc<dyn EmailSender>, String> {
    match config {
        EmailConfig::Log => Ok(std::sync::Arc::new(LogEmailSender)),
        EmailConfig::Smtp {
            host,
            port,
            username,
            password,
            from,
        } => {
            let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
                .map_err(|e| format!("Invalid SMTP configuration: {e}"))?
                .port(*port)
                .credentials(Credentials::new(username.clone(), password.clone()))
                .build();
            let from = from
                .parse::<Mailbox>()
                .map_err(|e| format!("Invalid SMTP_FROM: {e}"))?;
            Ok(std::sync::Arc::new(SmtpEmailSender { transport, from }))
        }
    }
}

struct LogEmailSender;

#[async_trait]
impl EmailSender for LogEmailSender {
    async fn send_verification(&self, recipient: &str, token: &str) -> Result<(), String> {
        tracing::warn!(
            recipient,
            token,
            "development email verification token; never use log email mode in production"
        );
        Ok(())
    }

    async fn send_password_reset(&self, recipient: &str, token: &str) -> Result<(), String> {
        tracing::warn!(
            recipient,
            token,
            "development password reset token; never use log email mode in production"
        );
        Ok(())
    }

    async fn send_invitation(
        &self,
        recipient: &str,
        project: &str,
        token: &str,
        cli_url: &str,
    ) -> Result<(), String> {
        tracing::warn!(
            recipient,
            project,
            token,
            cli_url,
            "development invitation token; never use log email mode in production"
        );
        Ok(())
    }
}

struct SmtpEmailSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

#[async_trait]
impl EmailSender for SmtpEmailSender {
    async fn send_verification(&self, recipient: &str, token: &str) -> Result<(), String> {
        let recipient = recipient
            .parse::<Mailbox>()
            .map_err(|e| format!("Invalid verification recipient: {e}"))?;
        let message = Message::builder()
            .from(self.from.clone())
            .to(recipient)
            .subject("Verify your Greenbyte email")
            .body(format!(
                "Enter this one-time token in the Greenbyte CLI to verify your email:\n\n{token}\n\nThis token expires soon and can be used once."
            ))
            .map_err(|e| format!("Could not build verification email: {e}"))?;
        self.transport
            .send(message)
            .await
            .map_err(|e| format!("Could not send verification email: {e}"))?;
        Ok(())
    }

    async fn send_password_reset(&self, recipient: &str, token: &str) -> Result<(), String> {
        let recipient = recipient
            .parse::<Mailbox>()
            .map_err(|e| format!("Invalid password reset recipient: {e}"))?;
        let message = Message::builder()
            .from(self.from.clone())
            .to(recipient)
            .subject("Reset your Greenbyte password")
            .body(format!(
                "Enter this one-time token in the Greenbyte CLI to reset your password:\n\n{token}\n\nIf you did not request this, ignore this email."
            ))
            .map_err(|e| format!("Could not build password reset email: {e}"))?;
        self.transport
            .send(message)
            .await
            .map_err(|e| format!("Could not send password reset email: {e}"))?;
        Ok(())
    }

    async fn send_invitation(
        &self,
        recipient: &str,
        project: &str,
        token: &str,
        cli_url: &str,
    ) -> Result<(), String> {
        let recipient = recipient
            .parse::<Mailbox>()
            .map_err(|e| format!("Invalid invitation recipient: {e}"))?;
        let body = format!(
            "You were invited to the Greenbyte project '{project}'.\n\n\
             Install the CLI from {cli_url}, run `greenbyte init {project}`, and enter this one-time token:\n\n\
             {token}\n\nThis token expires in 24 hours and can be used once. Obtain the project master key through a separate secure channel."
        );
        let message = Message::builder()
            .from(self.from.clone())
            .to(recipient)
            .subject(format!("Greenbyte invitation to {project}"))
            .body(body)
            .map_err(|e| format!("Could not build invitation email: {e}"))?;
        self.transport
            .send(message)
            .await
            .map_err(|e| format!("Could not send invitation email: {e}"))?;
        Ok(())
    }
}

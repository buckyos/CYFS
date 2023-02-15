use std::str::FromStr;
use lettre::{Message, SmtpTransport, Transport};
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use serde::Deserialize;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use log::*;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SmtpType {
    TLS,
    STARTTLS
}

impl Default for SmtpType {
    fn default() -> Self {
        Self::STARTTLS
    }
}

#[derive(Deserialize)]
pub struct EmailConfig {
    receiver: String,
    sender: String,
    smtp_server: String,
    password: String,
    #[serde(default)]
    smtp_type: SmtpType
}

impl EmailConfig {
    fn transport(&self) -> SmtpTransport {
        match self.smtp_type {
            SmtpType::TLS => {
                info!("send email use tls/ssl {}", &self.smtp_server);
                SmtpTransport::relay(&self.smtp_server)
            }
            SmtpType::STARTTLS => {
                info!("send email use starttls {}", &self.smtp_server);
                SmtpTransport::starttls_relay(&self.smtp_server)
            }
        }.unwrap().credentials(Credentials::new(
            self.sender.clone(),
            self.password.clone(),
        )).build()
    }
}

pub async fn send_mail(config: EmailConfig, subject: String, output_html: String) -> BuckyResult<()> {
    // 发送邮件
    let mail = Message::builder()
        .to(Mailbox::from_str(&config.receiver).unwrap())
        .from(Mailbox::from_str(&config.sender).unwrap())
        .subject(subject)
        .body(output_html)
        .unwrap();

    // Open connection to Gmail
    if let Err(e) = config.transport().send(&mail) {
        error!("Could not send email: {:?}", e);
        return Err(BuckyError::new(BuckyErrorCode::Failed, e.to_string()));
    }

    Ok(())
}
use std::str::FromStr;
use lettre::{Message, SmtpTransport, Transport};
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use serde::Deserialize;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use log::*;

#[derive(Deserialize)]
pub struct EmailConfig {
    receiver: String,
    sender: String,
    smtp_server: String,
    password: String,
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
    if let Err(e) = SmtpTransport::relay(&config.smtp_server)
        .unwrap()
        .credentials(Credentials::new(
            config.sender,
            config.password,
        )).build().send(&mail) {
        error!("Could not send email: {:?}", e);
        return Err(BuckyError::new(BuckyErrorCode::Failed, e.to_string()));
    }

    Ok(())
}
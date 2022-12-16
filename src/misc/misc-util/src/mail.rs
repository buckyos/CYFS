use lettre::{smtp::authentication::Credentials, SmtpClient, Transport};
use lettre_email::Email;
use serde::Deserialize;
use cyfs_base::BuckyResult;

#[derive(Deserialize)]
pub struct EmailConfig {
    receiver: String,
    sender: String,
    smtp_server: String,
    password: String,
}

pub struct MailReporter {
    receiver: String,
    sender: String,
    smtp_server: String,
    password: String,
}

impl MailReporter {
    pub async fn report(config: EmailConfig, output: String) -> BuckyResult<()> {
        let output = output.replace("\n", "<br>");
        // 发送邮件
        let builder = Email::builder()
        .to(config.receiver)
        .from(config.sender.clone())
        .subject(format!("{} Meta Chain Stat {}", cyfs_base::get_channel().to_string(), chrono::Local::today().format("%F")))
        .html(output).build().unwrap();

        // Open connection to Gmail
        let result = SmtpClient::new_simple(&config.smtp_server)
        .unwrap()
        .credentials(Credentials::new(
            config.sender,
            config.password,
        )).transport().send(builder.into());


        if result.is_ok() {
            info!("Email sent");
        } else {
            error!("Could not send email: {:?}", result);
        }

        Ok(())
    }
}
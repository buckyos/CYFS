use cyfs_base::*;
use crate::{def::*, EmailConfig};
use lettre::{smtp::authentication::Credentials, SmtpClient, Transport};
use lettre_email::Email;

extern crate lettre;
extern crate lettre_email;
extern crate mime;

pub struct Lettre {
    email_receiver: String,
    mine_email: String,
    smtp_server: String,
    password: String,
}

impl Lettre {
    pub fn new(config: &EmailConfig) -> Self {
        Self {
            email_receiver: config.email_receiver.to_owned(),
            mine_email: config.mine_email.to_owned(),
            smtp_server: config.smtp_server.to_owned(),
            password: config.password.to_owned(),
        }
    }
    
    pub async fn report(&self, info: &StatInfo) -> BuckyResult<()> {
        // 发送邮件        
        let mut email = Email::builder()
        .to(self.email_receiver.as_ref())
        .from(self.mine_email.as_ref())
        .subject("Meta Chain Stat")
        .html("<h1>Stat Metrics</h1>")
        .text(info.context.to_owned());

        for v in info.attachment.iter() {
            email = email.attachment_from_file(&std::path::Path::new(v.as_str()), None, &mime::IMAGE_PNG).unwrap();
        }

        let builder = email.build().unwrap();

        let creds = Credentials::new(
            self.mine_email.to_string(),
            self.password.to_string(),
        );

        // Open connection to Gmail
        let mut mailer = SmtpClient::new_simple(self.smtp_server.as_ref())
        .unwrap()
        .credentials(creds)
        .transport();

        // Send the email
        let result = mailer.send(builder.into());

        if result.is_ok() {
            info!("Email sent");
        } else {
            error!("Could not send email: {:?}", result);
        }

        //info!("{:?}", result);
        mailer.close();

        Ok(())
    }
}

#[async_trait::async_trait]
impl StatReporter for  Lettre {
    async fn report_stat(&self, info: &StatInfo) -> BuckyResult<()> {
        self.report(info).await
    }
}
use std::sync::Arc;
use chrono::DateTime;
use chrono::Datelike;
use chrono::Local;
use cyfs_base::*;
use crate::Config;
use crate::reporter::*;
use crate::def::*;
use crate::storage::{Storage, MetaStat};
use plotters::prelude::*;

#[derive(Clone, Debug, Copy)]
#[repr(u8)]
pub enum MetaDescObject {
    Device = 0,
    People = 1,
}

pub struct Client {
    storage: Arc<Box<dyn Storage + Send + Sync>>,
    deadline: u64,
    stat_reporter: Arc<StatReportManager>,
}

impl Client {
    pub(crate) fn new(config: &Config, storage: Arc<Box<dyn Storage + Send + Sync>>) -> Self {
        let stat_reporter = Arc::new(StatReportManager::new(&config));
        let deadline = config.deadline;
        Self {
            storage,
            deadline,
            stat_reporter,
        }
    }

    pub fn flow_chart(&self, data: Vec<(u64, u64)>, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        info!("data {:?}", data);
        let root = BitMapBackend::new(filename, (640, 480)).into_drawing_area();
        root.fill(&WHITE)?;
        let mut chart = ChartBuilder::on(&root)
            .caption(filename, ("sans-serif", 50).into_font())
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(0u64..1231u64, 0u64..1231u64)?;
    
        chart.configure_mesh().draw()?;
    
        chart
            .draw_series(LineSeries::new(
                data.iter().map(|x| (x.0, x.1) ).map(|x| (x.0, x.1)),
                &RED,
            ))?;
        chart
            .configure_series_labels()
            .background_style(&WHITE.mix(800f64))
            .border_style(&BLACK)
            .draw()?;
    
        root.present()?;

        Ok(())
    }

    pub async fn run(&self) {
        let mut stat_info = StatInfo {
            attachment: vec![],
            context: "".to_owned(),
        };
        if let Ok(ret) = self.get_desc().await {
            let t1 = format!("<p> Total Peoples: {}</p>", ret.0);
            stat_info.context += t1.as_str();
            stat_info.context += "\n\n";
            let t2 = format!("<p> Total Devices: {}</p>", ret.1);
            stat_info.context += t2.as_str();
            stat_info.context += "\n\n";
        }

        // object 查询 / api 调用情况
        if let Ok(ret) = self.meta_stat().await {
            let mut success_sum = 0;
            let mut failed_sum = 0;
            let mut count = 0;
            for v in ret.0.into_iter() {
                count += 1;
                success_sum += v.success;
                failed_sum  += v.failed;
            }
            let t1 = format!("<p> Last {} Days Query Meta Object: Num: {}, Success: {}, Failed:{}</p>", self.deadline, count, success_sum, failed_sum);
            stat_info.context += t1.as_str();
            stat_info.context += "\n";


            for v in ret.1.into_iter() {
                let t1 = format!("<p> Total Meta Api: {}, Success: {}, Failed:{}</p>", v.id, v.success.to_string(), v.failed.to_string());
                stat_info.context += t1.as_str();
                stat_info.context += "\n";
            }
        }

        stat_info.context += "\n\n";
        stat_info.context += "\n\n";

        // 日表
        if let Ok(ret) = self.period_stat(MetaDescObject::Device).await {

            for v in ret.0.iter() {
                let t1 = format!("<p> Date: {}, Device Add: {}</p>", v.0, v.1);
                stat_info.context += t1.as_str();
                stat_info.context += "\n";
            }

            for v in ret.1.iter() {
                let t1 = format!("<p> Date: {}, Device Active: {}</p>", v.0, v.1);
                stat_info.context += t1.as_str();
                stat_info.context += "\n";
            }

            let f1 = "device_daily_add.png";
            let f2 = "device_daily_active.png";
            let _ = self.flow_chart(ret.0, f1);
            let _ = self.flow_chart(ret.1, f2);

            //stat_info.attachment.push(f1.to_string());
            //stat_info.attachment.push(f2.to_string());

        }

        if let Ok(ret) = self.period_stat(MetaDescObject::People).await {
            for v in ret.0.iter() {
                let t1 = format!("<p> Date: {}, People Add: {}</p>", v.0, v.1);
                stat_info.context += t1.as_str();
                stat_info.context += "\n";
            }

            for v in ret.1.iter() {
                let t1 = format!("<p> Date: {}, People Active: {}</p>", v.0, v.1);
                stat_info.context += t1.as_str();
                stat_info.context += "\n";
            }

            let f1 = "people_daily_add.png";
            let f2 = "people_daily_active.png";
            let _ = self.flow_chart(ret.0, f1);
            let _ = self.flow_chart(ret.1, f2);
            //stat_info.attachment.push(f1.to_string());
            //stat_info.attachment.push(f2.to_string());
        }

        let _ = self.report(stat_info).await;

    }

    pub async fn get_desc(&self) -> BuckyResult<(u64, u64)> {
        let total_devices = self.storage.get_desc(0 as u8).await?;
        let total_peoples = self.storage.get_desc(1 as u8).await?;

        Ok((total_peoples, total_devices))
    }
    
    // FIXME: 默认取当前日期
    pub async fn period_stat(&self, obj_type: MetaDescObject) -> BuckyResult<(Vec<(u64, u64)>, Vec<(u64, u64)>)> {

        let mut add = Vec::new();
        let mut active = Vec::new();
        
        let mut end = bucky_time_to_js_time(bucky_time_now());
        end = js_time_to_bucky_time(end - ((end  % (86400 * 1000))));
        let start = end;
        for j in 0..=self.deadline {
            let end_js = bucky_time_to_js_time(end) - (j -1) * 86400 * 1000;
    
            let bucky_end = js_time_to_bucky_time(end_js);

            let start_js = bucky_time_to_js_time(start) - j * 86400 * 1000;
            let bucky_start = js_time_to_bucky_time(start_js);

            let date_end = js_time_to_bucky_time(end_js - 86400 * 1000);
            let sys_time = bucky_time_to_system_time(date_end);
            let datetime: DateTime<Local> = sys_time.into();

            let x_axis = format!("{:02}{:02}", datetime.month(), datetime.day()).parse::<u64>().unwrap(); 

            let sum = self.storage.get_desc_add(obj_type as u8, bucky_start, bucky_end).await?;
            add.push((x_axis, sum));

            let sum = self.storage.get_desc_active(obj_type as u8, bucky_start, bucky_end).await?;
            active.push((x_axis, sum));

        }

        add.reverse();
        active.reverse();

        Ok((add, active))
    }

    pub async fn meta_stat(&self) -> BuckyResult<(Vec<MetaStat>, Vec<MetaStat>)> {
        let now = bucky_time_now();

        let mut start = bucky_time_to_js_time(now);
        let end = js_time_to_bucky_time(start);
        start -= self.deadline * 86400 * 1000;
        let start = js_time_to_bucky_time(start);
        let v1 = self.storage.get_meta_stat(0u8, start, end).await?;
        let v2 = self.storage.get_meta_stat(1u8, start, end).await?;
        Ok((v1, v2))
    }

    pub async fn report(&self, stat: StatInfo) -> BuckyResult<()> {
        self.stat_reporter.report(&StatInfo {
            attachment: stat.attachment,
            context: stat.context,
        }).await
    }

}
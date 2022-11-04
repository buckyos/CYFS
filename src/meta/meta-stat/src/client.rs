use std::sync::Arc;
use std::collections::HashMap;
use cyfs_base::*;
use crate::Config;
use crate::reporter::*;
use crate::def::*;
use crate::storage::{Storage, MetaStat};
use comfy_table::Table;
use plotters::prelude::*;


#[derive(Clone, Debug)]
pub enum MetaDescObject {
    Device,
    People,
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
        let root = BitMapBackend::new(filename, (640, 480)).into_drawing_area();
        root.fill(&WHITE)?;
        let mut chart = ChartBuilder::on(&root)
            .caption(filename, ("sans-serif", 50).into_font())
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(0f32..10000f32, 0f32..10000f32)?;
    
        chart.configure_mesh().draw()?;
    
        chart
            .draw_series(LineSeries::new(
                data.into_iter().map(|x| (x.0 as f32, x.1 as f32) ).map(|x| (x.0, x.1)),
                &RED,
            ))?
            .label(filename)
            .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], &RED));
    
        chart
            .configure_series_labels()
            .background_style(&WHITE.mix(0.8))
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
        // 概况
        let mut table = Table::new();
        table.set_header(vec!["Total People", "Total Device"]);
        if let Ok(ret) = self.get_desc().await {
            let ret: Vec<u64> = ret.into_iter().map(|v| v.1).collect();
            table.add_row(ret);
            println!("{table}");
        }

        let t1 = format!("{table}");

        let mut table1 = Table::new();
        table1.set_header(vec!["Query Meta Object", "Success", "Failed"]);

        let mut table2 = Table::new();
        table2.set_header(vec!["Call Meta Api", "Success", "Failed"]);
        // object 查询 / api 调用情况
        if let Ok(ret) = self.meta_stat().await {
            for v in ret.0.into_iter() {
                table1.add_row(vec![v.id, v.success.to_string(), v.failed.to_string()]);
            }

            for v in ret.1.into_iter() {
                table2.add_row(vec![v.id, v.success.to_string(), v.failed.to_string()]);
            }
        }
        println!("{table1}");
        println!("{table2}");

        // 日表
        if let Ok(ret) = self.period_stat(MetaDescObject::Device).await {
            let _ = self.flow_chart(ret.0, "device_daily_add.png");
            let _ = self.flow_chart(ret.1, "device_daily_active.png");
        }

        if let Ok(ret) = self.period_stat(MetaDescObject::People).await {
            let _ = self.flow_chart(ret.0, "people_daily_add.png");
            let _ = self.flow_chart(ret.1, "people_daily_active.png");
        }

        let _ = self.report(&stat_info).await;

    }

    pub async fn get_desc(&self) -> BuckyResult<HashMap<u8, u64>> {
        let mut ret = HashMap::new();
        for i in 0..2 {
            let sum = self.storage.get_desc(i as u8).await?;
            ret.insert(i, sum);
        }
        Ok(ret)
    }
    
    // FIXME: 默认取当前日期
    pub async fn period_stat(&self, obj_type: MetaDescObject) -> BuckyResult<(Vec<(u64, u64)>, Vec<(u64, u64)>)> {
        let now = bucky_time_now();

        let mut add = Vec::new();
        let mut active = Vec::new();

        for j in 1..=self.deadline {
            let mut start = bucky_time_to_js_time(now);
            let end = js_time_to_bucky_time(start);
            start -= j * 86400 * 1000;
            let start = js_time_to_bucky_time(start);

            let sum = self.storage.get_desc_add(obj_type as u8, start, end).await?;
            add.push((end, sum));

            let sum = self.storage.get_desc_active(obj_type as u8, start, end).await?;
            active.push((end, sum));
        }

        add.reverse();
        active.reverse();

        Ok((add, active))
    }

    pub async fn meta_stat(&self) -> BuckyResult<(Vec<MetaStat>, Vec<MetaStat>)> {
        let now = bucky_time_now();

        let mut start = bucky_time_to_js_time(now);
        let end = js_time_to_bucky_time(start);
        start -= 30 * 86400 * 1000;
        let start = js_time_to_bucky_time(start);
        let v1 = self.storage.get_meta_stat(0u8, start, end).await?;
        let v2 = self.storage.get_meta_stat(1u8, start, end).await?;
        Ok((v1, v2))
    }

    pub async fn report(&self, stat: &StatInfo) -> BuckyResult<()> {
        self.stat_reporter.report(&StatInfo {
            attachment: vec![],
            context: "Stat info".to_string(),
        }).await
    }

}
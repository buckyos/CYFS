use std::str::FromStr;
use cyfs_base::*;
use cyfs_task_manager::{TaskId, TaskStatus};
use crate::trans_api::{sql_query, SqlConnection, SqlRow};

#[async_trait::async_trait]
pub trait DownloadTaskTracker {
    async fn add_task_info(&mut self, task_id: &TaskId, context_id: &Option<ObjectId>, task_status: TaskStatus, dec_list: Vec<(DeviceId, ObjectId)>) -> BuckyResult<()>;
    async fn set_task_status(&mut self, task_id: &TaskId, task_status: TaskStatus) -> BuckyResult<()>;
    async fn get_tasks(&mut self, source: &DeviceId, dec_id: &ObjectId, context_id: &Option<ObjectId>, task_status: Option<TaskStatus>, range: Option<(u64, u32)>) -> BuckyResult<Vec<TaskId>>;
    async fn remove_task_info(&mut self, source: &DeviceId, dec_id: &ObjectId, task_id: &TaskId) -> BuckyResult<()>;
}

#[async_trait::async_trait]
impl DownloadTaskTracker for SqlConnection {
    async fn add_task_info(&mut self, task_id: &TaskId, context_id: &Option<ObjectId>, task_status: TaskStatus, dec_list: Vec<(DeviceId, ObjectId)>) -> BuckyResult<()> {
        info!("task tracker add task: id={}, context={:?}, dec_list={:?}", task_id, context_id, dec_list);
        
        for (source, dec) in dec_list.iter() {
            let sql = r#"insert into download_task_tracker (source, dec_id, task_id, context_id, task_status) values (?1, ?2, ?3, ?4, ?5)"#;
            let context_id = if context_id.is_some() {
                context_id.as_ref().unwrap().to_string()
            } else {
                "null".to_string()
            };
            self.execute_sql(sql_query(sql)
                .bind(source.to_string())
                .bind(dec.to_string())
                .bind(task_id.to_string())
                .bind(context_id)
                .bind(task_status.into())).await?;
        }
        Ok(())
    }

    async fn set_task_status(&mut self, task_id: &TaskId, task_status: TaskStatus) -> BuckyResult<()> {
        let sql = r#"update download_task_tracker set task_status = ?1 where task_id = ?2"#;
        self.execute_sql(sql_query(sql).bind(task_status.into()).bind(task_id.to_string())).await?;
        Ok(())
    }

    async fn get_tasks(&mut self, source: &DeviceId, dec_id: &ObjectId, context_id: &Option<ObjectId>, task_status: Option<TaskStatus>, range: Option<(u64, u32)>) -> BuckyResult<Vec<TaskId>> {
        let rows = if context_id.is_some() && task_status.is_some() && range.is_some() {
            let sql = r#"select task_id from download_task_tracker where source = ?6 and dec_id = ?1 and context_id = ?2 and task_status = ?3 order by created_at desc limit ?4, ?5"#;
            self.query_all(sql_query(sql)
                .bind(dec_id.to_string())
                .bind(context_id.as_ref().unwrap().to_string())
                .bind(task_status.unwrap().into())
                .bind(range.as_ref().unwrap().0 as i64)
                .bind(range.as_ref().unwrap().1 as i32)
                .bind(source.to_string())).await?
        } else if context_id.is_some() && task_status.is_some() {
            let sql = r#"select task_id from download_task_tracker where source = ?4 and dec_id = ?1 and context_id = ?2 and task_status = ?3 order by created_at desc"#;
            self.query_all(sql_query(sql)
                .bind(dec_id.to_string())
                .bind(context_id.as_ref().unwrap().to_string())
                .bind(task_status.unwrap().into())
                .bind(source.to_string())).await?
        } else if context_id.is_some() && range.is_some() {
            let sql = r#"select task_id from download_task_tracker where source = ?5 and dec_id = ?1 and context_id = ?2 order by created_at desc limit ?3, ?4"#;
            self.query_all(sql_query(sql)
                .bind(dec_id.to_string())
                .bind(context_id.as_ref().unwrap().to_string())
                .bind(range.as_ref().unwrap().0 as i64)
                .bind(range.as_ref().unwrap().1 as i32)
                .bind(source.to_string())).await?
        } else if task_status.is_some() && range.is_some() {
            let sql = r#"select task_id from download_task_tracker where source = ?5 and dec_id = ?1 and task_status = ?2 order by created_at desc limit ?3, ?4"#;
            self.query_all(sql_query(sql)
                .bind(dec_id.to_string())
                .bind(task_status.unwrap().into())
                .bind(range.as_ref().unwrap().0 as i64)
                .bind(range.as_ref().unwrap().1 as i32)
                .bind(source.to_string())).await?
        } else if context_id.is_some() {
            let sql = r#"select task_id from download_task_tracker where source = ?3 and dec_id = ?1 and context_id = ?2 order by created_at desc"#;
            self.query_all(sql_query(sql)
                .bind(dec_id.to_string())
                .bind(context_id.as_ref().unwrap().to_string())
                .bind(source.to_string())).await?
        } else if task_status.is_some() {
            let sql = r#"select task_id from download_task_tracker where source = ?3 and dec_id = ?1 and task_status = ?2 order by created_at desc"#;
            self.query_all(sql_query(sql)
                .bind(dec_id.to_string())
                .bind(task_status.unwrap().into())
                .bind(source.to_string())).await?
        } else if range.is_some() {
            let sql = r#"select task_id from download_task_tracker where source = ?4 and dec_id = ?1 order by created_at desc limit ?2, ?3"#;
            self.query_all(sql_query(sql)
                .bind(dec_id.to_string())
                .bind(range.as_ref().unwrap().0 as i64)
                .bind(range.as_ref().unwrap().1 as i32)
                .bind(source.to_string())).await?
        } else {
            let sql = r#"select task_id from download_task_tracker where dec_id = ?1 and source = ?2 order by created_at desc"#;
            self.query_all(sql_query(sql)
                .bind(dec_id.to_string())
                .bind(source.to_string())).await?
        };

        let mut list = Vec::new();
        for row in rows {
            list.push(TaskId::from_str(row.get("task_id"))?);
        }
        Ok(list)
    }

    async fn remove_task_info(&mut self, source: &DeviceId, dec_id: &ObjectId, task_id: &TaskId) -> BuckyResult<()> {
        let sql = r#"delete from download_task_tracker where task_id = ?1 and dec_id = ?2 and source = ?3"#;
        self.execute_sql(sql_query(sql)
            .bind(task_id.to_string())
            .bind(dec_id.to_string())
            .bind(source.to_string())).await?;
        Ok(())
    }
}

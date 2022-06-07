use std::path::{Path};
use std::str::FromStr;
use cyfs_base::*;
use crate::{DecInfo, sql_query, SqlConnection, SqlPool, TaskCategory, TaskId, TaskStatus, TaskType, SqlRow, RawSqlPool};

#[async_trait::async_trait]
pub trait TaskStore: Send + Sync {
    async fn save_task(&self, task_id: &TaskId, task_status: TaskStatus, task_data: Vec<u8>) -> BuckyResult<()>;
    async fn save_task_status(&self, task_id: &TaskId, task_status: TaskStatus) -> BuckyResult<()>;
    async fn save_task_data(&self, task_id: &TaskId, task_data: Vec<u8>) -> BuckyResult<()>;
}

pub struct QueryTaskParams {
    source: DeviceId,
    dec_id: Option<ObjectId>,
}
#[async_trait::async_trait]
pub trait TaskManagerStore: Send + Sync {
    async fn add_task(&self, task_id: &TaskId, category: TaskCategory, task_type: TaskType, task_status: TaskStatus, dec_list: Vec<DecInfo>, task_params: Vec<u8>) -> BuckyResult<()>;
    async fn get_task(&self, task_id: &TaskId) -> BuckyResult<(TaskCategory, TaskType, TaskStatus, Vec<u8>, Vec<u8>)>;
    async fn get_tasks_by_status(&self, status: TaskStatus) -> BuckyResult<Vec<(TaskId, TaskType, Vec<u8>, Vec<u8>)>>;
    async fn get_tasks_by_category(&self, category: TaskCategory) -> BuckyResult<Vec<(TaskId, TaskType, TaskStatus, Vec<u8>, Vec<u8>)>>;
    async fn get_tasks_by_task_id(&self, task_id_list: &[TaskId]) -> BuckyResult<Vec<(TaskId, TaskType, TaskStatus, Vec<u8>, Vec<u8>)>>;
    async fn get_tasks(&self, source: &DeviceId, dec_id: &ObjectId, category: TaskCategory, task_status: TaskStatus, range: Option<(u64, u32)>) -> BuckyResult<Vec<(TaskId, TaskType, TaskStatus, Vec<u8>, Vec<u8>)>>;
    async fn get_dec_list(&self, task_id: &TaskId) -> BuckyResult<Vec<DecInfo>>;
    async fn add_dec_info(&self, task_id: &TaskId, category: TaskCategory, task_status: TaskStatus, dec_info: &DecInfo) -> BuckyResult<()>;
    async fn delete_dec_info(&self, task_id: &TaskId, dec_id: &ObjectId, source: &DeviceId) -> BuckyResult<()>;
    async fn delete_task(&self, task_id: &TaskId) -> BuckyResult<()>;
}

pub struct SQLiteTaskStore {
    pool: SqlPool
}

impl From<RawSqlPool> for SQLiteTaskStore {
    fn from(pool: RawSqlPool) -> Self {
        Self {
            pool: SqlPool::from_raw_pool(pool)
        }
    }
}

impl SQLiteTaskStore {
    pub async fn new<P: AsRef<Path>>(db_path: P) -> BuckyResult<Self> {
        let pool = SqlPool::open(format!("sqlite://{}", db_path.as_ref().to_string_lossy().to_string()).as_str(), 10).await?;
        Ok(Self {
            pool
        })
    }

    pub async fn create_connection(&self) -> BuckyResult<SqlConnection> {
        self.pool.get_conn().await
    }

    pub async fn init(&self) -> BuckyResult<()> {
        let mut conn = self.pool.get_conn().await?;
        let sql = r#"create table if not exists "tasks" (
            "task_id" char(45) primary key not null,
            "task_category" INTEGER,
            "task_type" INTEGER,
            "task_status" INTEGER,
            "task_param" BLOB,
            "task_data" BLOB,
            "created_at" TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            "updated_at" TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )"#;
        conn.execute_sql(sql_query(sql)).await?;

        let sql = r#"create index if not exists category_index on tasks (task_category, created_at)"#;
        conn.execute_sql(sql_query(sql)).await?;

        let sql = r#"create index if not exists status_index on tasks (task_status, updated_at)"#;
        conn.execute_sql(sql_query(sql)).await?;

        let sql = r#"create table if not exists "dec_tasks" (
            "source" char(45) not null,
            "dec_id" char(45) not null,
            "task_id" char(45) not null,
            "task_status" INTEGER,
            "task_category" INTEGER,
            "task_type" INTEGER,
            "dec_info" BLOB not null,
            "created_at" TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            "updated_at" TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )"#;
        conn.execute_sql(sql_query(sql)).await?;

        let sql = r#"create index if not exists dec_index on dec_tasks (source, dec_id, task_category, task_status, created_at, task_id)"#;
        conn.execute_sql(sql_query(sql)).await?;

        let sql = r#"create index if not exists task_index on dec_tasks (task_id)"#;
        conn.execute_sql(sql_query(sql)).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl TaskStore for SQLiteTaskStore {
    async fn save_task(&self, task_id: &TaskId, task_status: TaskStatus, task_data: Vec<u8>) -> BuckyResult<()> {
        let mut conn = self.pool.get_conn().await?;
        conn.begin_transaction().await?;

        let sql = r#"update tasks set task_status = ?1, task_data = ?2, updated_at = CURRENT_TIMESTAMP where task_id = ?3"#;
        conn.execute_sql(sql_query(sql).bind(task_status.into()).bind(task_data).bind(task_id.to_string())).await?;

        let sql = r#"update dec_tasks set task_status = ?1, updated_at = CURRENT_TIMESTAMP where task_id = ?2"#;
        conn.execute_sql(sql_query(sql).bind(task_status.into()).bind(task_id.to_string())).await?;

        conn.commit_transaction().await?;
        Ok(())
    }

    async fn save_task_status(&self, task_id: &TaskId, task_status: TaskStatus) -> BuckyResult<()> {
        let mut conn = self.pool.get_conn().await?;
        conn.begin_transaction().await?;
        let sql = r#"update tasks set task_status = ?1, updated_at = CURRENT_TIMESTAMP where task_id = ?2"#;
        conn.execute_sql(sql_query(sql).bind(task_status.into()).bind(task_id.to_string())).await?;

        let sql = r#"update dec_tasks set task_status = ?1, updated_at = CURRENT_TIMESTAMP where task_id = ?2"#;
        conn.execute_sql(sql_query(sql).bind(task_status.into()).bind(task_id.to_string())).await?;

        conn.commit_transaction().await?;
        Ok(())
    }

    async fn save_task_data(&self, task_id: &TaskId, task_data: Vec<u8>) -> BuckyResult<()> {
        let mut conn = self.pool.get_conn().await?;
        let sql = r#"update tasks set task_data = ?1 where task_id = ?2"#;
        conn.execute_sql(sql_query(sql).bind(task_data).bind(task_id.to_string())).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl TaskManagerStore for SQLiteTaskStore
{
    async fn add_task(&self, task_id: &TaskId, category: TaskCategory, task_type: TaskType, task_status: TaskStatus, dec_list: Vec<DecInfo>, task_params: Vec<u8>) -> BuckyResult<()> {
        let mut conn = self.pool.get_conn().await?;
        conn.begin_transaction().await?;

        let sql = r#"insert into tasks (task_id, task_category, task_type, task_status, task_param, task_data) values (?1, ?2, ?3, ?4, ?5, ?6)"#;
        conn.execute_sql(sql_query(sql)
            .bind(task_id.to_string())
            .bind(category.into())
            .bind(task_type.into())
            .bind(task_status.into())
            .bind(task_params)
            .bind(Vec::new())).await?;

        for dec_info in dec_list.iter() {
            let sql = r#"insert into dec_tasks (source, dec_id, task_category, task_status, task_id, dec_info) values (?1, ?2, ?3, ?4, ?5, ?6)"#;
            conn.execute_sql(sql_query(sql)
                .bind(dec_info.source().to_string())
                .bind(dec_info.dec_id().to_string())
                .bind(category.into())
                .bind(task_status.into())
                .bind(task_id.to_string())
                .bind(dec_info.to_vec()?)).await?;
        }

        conn.commit_transaction().await?;

        Ok(())
    }

    async fn get_task(&self, task_id: &TaskId) -> BuckyResult<(TaskCategory, TaskType, TaskStatus, Vec<u8>, Vec<u8>)> {
        let mut conn = self.pool.get_conn().await?;
        let sql = r#"select * from tasks where task_id = ?1"#;
        let row = conn.query_one(sql_query(sql).bind(task_id.to_string())).await?;
        Ok((TaskCategory::try_from(row.get("task_category"))?,
            TaskType::try_from(row.get("task_type"))?,
            TaskStatus::try_from(row.get("task_status"))?,
            row.get("task_param"), row.get("task_data")))
    }

    async fn get_tasks_by_status(&self, status: TaskStatus) -> BuckyResult<Vec<(TaskId, TaskType, Vec<u8>, Vec<u8>)>> {
        let mut conn = self.pool.get_conn().await?;
        let sql = r#"select * from tasks where task_status = ?1"#;
        let rows = conn.query_all(sql_query(sql).bind(status.into())).await?;
        let mut list = Vec::new();
        for row in rows.iter() {
            list.push((TaskId::from_str(row.get("task_id"))?,
                       TaskType::try_from(row.get("task_type"))?,
                       row.get("task_param"), row.get("task_data")))
        }
        Ok(list)
    }

    async fn get_tasks_by_category(&self, category: TaskCategory) -> BuckyResult<Vec<(TaskId, TaskType, TaskStatus, Vec<u8>, Vec<u8>)>> {
        let mut conn = self.pool.get_conn().await?;
        let sql = r#"select * from tasks where task_category = ?1"#;
        let rows = conn.query_all(sql_query(sql).bind(category.into())).await?;
        let mut list = Vec::new();
        for row in rows.iter() {
            list.push((TaskId::from_str(row.get("task_id"))?,
                       TaskType::try_from(row.get("task_type"))?,
                       TaskStatus::try_from(row.get("task_status"))?,
                       row.get("task_param"), row.get("task_data")))
        }
        Ok(list)
    }

    async fn get_tasks_by_task_id(&self, task_id_list: &[TaskId]) -> BuckyResult<Vec<(TaskId, TaskType, TaskStatus, Vec<u8>, Vec<u8>)>> {
        let mut conn = self.pool.get_conn().await?;

        let mut remainder = task_id_list;
        let mut list = Vec::new();
        while remainder.len() > 0 {
            let (left, right) = if remainder.len() > 100 {
                remainder.split_at(100)
            } else {
                (remainder, &remainder[remainder.len()..])
            };
            remainder = right;
            let id_list: Vec<String> = left.iter().map(|task_id| {
                format!("'{}'", task_id.to_string())
            }).collect();
            let in_sql = id_list.join(",");

            let sql = format!(r#"select * from tasks where task_id in ({})"#, in_sql);
            let rows = conn.query_all(sql_query(sql.as_str())).await?;
            for row in rows.iter() {
                list.push((TaskId::from_str(row.get("task_id"))?,
                           TaskType::try_from(row.get("task_type"))?,
                           TaskStatus::try_from(row.get("task_status"))?,
                           row.get("task_param"), row.get("task_data")))
            }
        }
        Ok(list)
    }

    async fn get_tasks(&self, source: &DeviceId, dec_id: &ObjectId, category: TaskCategory, task_status: TaskStatus, range: Option<(u64, u32)>) -> BuckyResult<Vec<(TaskId, TaskType, TaskStatus, Vec<u8>, Vec<u8>)>> {
        let mut conn = self.pool.get_conn().await?;

        let rows = if range.is_none() {
            let sql = r#"select task_id from dec_tasks where source = ?1 and dec_id = ?2 and category = ?3 and task_status = ?4 order by created_at"#;
            conn.query_all(sql_query(sql)
                .bind(source.to_string())
                .bind(dec_id.to_string())
                .bind(category.into())
                .bind(task_status.into())).await?
        } else {
            let sql = r#"select task_id from dec_tasks where source = ?1 and dec_id = ?2 and category = ?3 and task_status = ?4 order by created_at limit ?5, ?6"#;
            conn.query_all(sql_query(sql)
                .bind(source.to_string())
                .bind(dec_id.to_string())
                .bind(category.into())
                .bind(task_status.into())
                .bind(range.as_ref().unwrap().0 as i64)
                .bind(range.as_ref().unwrap().1 as i32)).await?
        };

        let mut task_id_list = Vec::new();
        for row in rows {
            task_id_list.push(TaskId::from_str(row.get("task_id"))?);
        }

        let mut remainder = task_id_list.as_slice();
        let mut list = Vec::new();
        while remainder.len() > 0 {
            let (left, right) = if remainder.len() > 100 {
                remainder.split_at(100)
            } else {
                (remainder, &remainder[remainder.len()..])
            };
            remainder = right;
            let id_list: Vec<String> = left.iter().map(|task_id| {
                format!("'{}'", task_id.to_string())
            }).collect();
            let in_sql = id_list.join(",");

            let sql = format!(r#"select * from tasks where task_id in ({})"#, in_sql);
            let rows = conn.query_all(sql_query(sql.as_str())).await?;
            for row in rows.iter() {
                list.push((TaskId::from_str(row.get("task_id"))?,
                           TaskType::try_from(row.get("task_type"))?,
                           TaskStatus::try_from(row.get("task_status"))?,
                           row.get("task_param"), row.get("task_data")))
            }
        }
        Ok(list)
    }

    async fn get_dec_list(&self, task_id: &TaskId) -> BuckyResult<Vec<DecInfo>> {
        let mut conn = self.pool.get_conn().await?;
        let sql = r#"select dec_info from dec_tasks where task_id = ?1"#;
        let rows = conn.query_all(sql_query(sql).bind(task_id.to_string())).await?;
        let mut list = Vec::new();
        for row in rows {
            list.push(DecInfo::clone_from_slice(row.get("dec_info"))?);
        }
        Ok(list)
    }

    async fn add_dec_info(&self, task_id: &TaskId, category: TaskCategory, task_status: TaskStatus, dec_info: &DecInfo) -> BuckyResult<()> {
        let sql = r#"insert into dec_tasks (source, dec_id, task_category, task_status, task_id, dec_info) values (?1, ?2, ?3, ?4, ?5, ?6)"#;
        let mut conn = self.pool.get_conn().await?;
        conn.execute_sql(sql_query(sql)
            .bind(dec_info.source().to_string())
            .bind(dec_info.dec_id().to_string())
            .bind(category.into())
            .bind(task_status.into())
            .bind(task_id.to_string())
            .bind(dec_info.to_vec()?)).await?;
        Ok(())
    }

    async fn delete_dec_info(&self, task_id: &TaskId, dec_id: &ObjectId, source: &DeviceId) -> BuckyResult<()> {
        let sql = r#"delete from dec_tasks where task_id = ?1 and dec_id = ?2 and source = ?3"#;
        let mut conn = self.pool.get_conn().await?;
        conn.execute_sql(sql_query(sql)
            .bind(task_id.to_string())
            .bind(dec_id.to_string())
            .bind(source.to_string())).await?;
        Ok(())
    }

    async fn delete_task(&self, task_id: &TaskId) -> BuckyResult<()> {
        let mut conn = self.pool.get_conn().await?;
        conn.begin_transaction().await?;

        let sql = r#"delete from tasks where task_id = ?1"#;
        conn.execute_sql(sql_query(sql).bind(task_id.to_string())).await?;

        let sql = r#"delete from dec_tasks where task_id = ?1"#;
        conn.execute_sql(sql_query(sql).bind(task_id.to_string())).await?;

        conn.commit_transaction().await?;

        Ok(())
    }
}

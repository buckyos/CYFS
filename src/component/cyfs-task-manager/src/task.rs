use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use async_std::task::JoinHandle;
use base58::{FromBase58, ToBase58};
use futures::future::{AbortHandle};
use generic_array::GenericArray;
use generic_array::typenum::{U32};
use cyfs_base::*;
use crate::{AsyncCondvar, AsyncCondvarRef, TaskStore};

pub const PUBLISH_TASK_CATEGORY: TaskCategory = TaskCategory(1);
pub const DOWNLOAD_TASK_CATEGORY: TaskCategory = TaskCategory(2);
pub const BUILD_FILE_TASK_CATEGORY: TaskCategory = TaskCategory(3);

pub const PUBLISH_LOCAL_FILE_TASK: TaskType = TaskType(101);
pub const PUBLISH_LOCAL_DIR_TASK: TaskType = TaskType(102);
pub const DOWNLOAD_CHUNK_TASK: TaskType = TaskType(111);
pub const DOWNLOAD_FILE_TASK: TaskType = TaskType(112);
pub const BUILD_FILE_TASK: TaskType = TaskType(121);
pub const BUILD_DIR_TASK: TaskType = TaskType(122);

#[derive(Copy, Clone, Eq, PartialEq, Debug, RawEncode, RawDecode)]
pub enum TaskStatus {
    Stopped,
    Paused,
    Running,
    Finished,
    Failed,
}

impl TaskStatus {
    pub fn into(self) -> i32 {
        match self {
            Self::Stopped => 0,
            Self::Paused => 1,
            Self::Running => 2,
            Self::Finished => 3,
            Self::Failed => 4,
        }
    }

    pub fn try_from(value: i32) -> BuckyResult<Self> {
        match value {
            0 => Ok(Self::Stopped),
            1 => Ok(Self::Paused),
            2 => Ok(Self::Running),
            3 => Ok(Self::Finished),
            4 => Ok(Self::Failed),
            _ => {
                let msg = format!("unsupport task type {}", value);
                log::error!("{}", msg.as_str());
                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct TaskType(pub u16);

impl TaskType {
    pub fn into(self) -> i32 {
        self.0 as i32
    }

    pub fn try_from(value: i32) -> BuckyResult<Self> {
        Ok(Self(value as u16))
    }
}

impl Display for TaskType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct TaskCategory(pub u16);

impl TaskCategory {
    pub fn into(self) -> i32 {
        self.0 as i32
    }
    pub fn try_from(value: i32) -> BuckyResult<Self> {
        Ok(Self(value as u16))
    }
}

impl Display for TaskCategory {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Copy, Clone, PartialOrd, PartialEq, Ord, Eq, Debug, Default)]
pub struct TaskId(GenericArray<u8, U32>);

impl From<&[u8]> for TaskId {
    fn from(hash: &[u8]) -> Self {
        Self(GenericArray::clone_from_slice(hash))
    }
}

impl Display for TaskId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.as_slice().to_base58())
    }
}

impl FromStr for TaskId {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let buf = s.from_base58().map_err(|_e| {
            log::error!("convert base58 str to TaskId failed, str:{}", s);
            let msg = format!("convert base58 str to object id failed, str={}", s);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        if buf.len() != 32 {
            let msg = format!(
                "convert base58 str to object id failed, len unmatch: str={}",
                s
            );
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let mut id = Self::default();
        unsafe {
            std::ptr::copy(buf.as_ptr(), id.0.as_mut_slice().as_mut_ptr(), buf.len());
        }

        Ok(id)
    }
}

impl Hash for TaskId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.0.as_slice());
    }
}

impl TaskId {
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl From<GenericArray<u8, U32>> for TaskId {
    fn from(hash: GenericArray<u8, U32>) -> Self {
        Self(hash)
    }
}

#[async_trait::async_trait]
pub trait Task: Send + Sync {
    fn get_task_id(&self) -> TaskId;
    fn get_task_type(&self) -> TaskType;
    fn get_task_category(&self) -> TaskCategory;
    fn need_persist(&self) -> bool {
        true
    }
    async fn get_task_status(&self) -> TaskStatus;
    async fn set_task_store(&mut self, task_store: Arc<dyn TaskStore>);
    async fn start_task(&self) -> BuckyResult<()>;
    async fn pause_task(&self) -> BuckyResult<()>;
    async fn stop_task(&self) -> BuckyResult<()>;
    async fn check_and_waiting_stop(&self) {
        loop {
            if TaskStatus::Running == self.get_task_status().await {
                async_std::task::sleep(Duration::from_secs(1)).await;
            } else {
                break;
            }
        }
    }
    async fn get_task_detail_status(&self) -> BuckyResult<Vec<u8>>;
}

#[async_trait::async_trait]
pub trait Runnable: Send + Sync {
    fn get_task_id(&self) -> TaskId;
    fn get_task_type(&self) -> TaskType;
    fn get_task_category(&self) -> TaskCategory;
    fn need_persist(&self) -> bool {
        true
    }
    fn status_change(&self, _task_status: TaskStatus) {}
    async fn set_task_store(&mut self, task_store: Arc<dyn TaskStore>);
    async fn run(&self) -> BuckyResult<()>;
    async fn get_task_detail_status(&self) -> BuckyResult<Vec<u8>>;
}

struct RunnableTaskData {
    canceler: Option<AbortHandle>,
    task_status: TaskStatus,
    task_store: Option<Arc<dyn TaskStore>>,
    runnable_handle: Option<JoinHandle<()>>,
}

pub struct RunnableTask<R: Runnable> {
    runnable: Arc<R>,
    data: Arc<Mutex<RunnableTaskData>>,
    waiting_list: Arc<Mutex<Vec<AsyncCondvarRef>>>,
}

impl<R: Runnable> RunnableTask<R> {
    pub fn new(runnable: R) -> Self {
        Self {
            runnable: Arc::new(runnable),
            data: Arc::new(Mutex::new(RunnableTaskData {
                canceler: None,
                task_status: TaskStatus::Stopped,
                task_store: None,
                runnable_handle: None,
            })),
            waiting_list: Arc::new(Mutex::new(vec![]))
        }
    }

    fn get_runnable(&self) -> &mut dyn Runnable {
        unsafe {
            let runnable = &mut *(self.runnable.as_ref() as *const dyn Runnable as *mut dyn Runnable);
            runnable
        }
    }
}

#[async_trait::async_trait]
impl<R: 'static + Runnable> Task for RunnableTask<R> {
    fn get_task_id(&self) -> TaskId {
        self.runnable.get_task_id()
    }

    fn get_task_type(&self) -> TaskType {
        self.runnable.get_task_type()
    }

    fn get_task_category(&self) -> TaskCategory {
        self.runnable.get_task_category()
    }

    fn need_persist(&self) -> bool {
        self.runnable.need_persist()
    }

    async fn get_task_status(&self) -> TaskStatus {
        let data = self.data.lock().unwrap();
        data.task_status
    }

    async fn set_task_store(&mut self, task_store: Arc<dyn TaskStore>) {
        {
            let mut data = self.data.lock().unwrap();
            data.task_store = Some(task_store.clone());
        }

        let runnable = self.get_runnable();
        runnable.set_task_store(task_store).await;
    }

    async fn start_task(&self) -> BuckyResult<()> {
        let runnable = self.runnable.clone();
        let task_id = self.runnable.get_task_id();

        {
            let tmp_data = self.data.lock().unwrap();
            if tmp_data.task_status == TaskStatus::Running {
                return Ok(());
            }
        }

        let (ft, handle) = futures::future::abortable(async move {
            runnable.run().await
        });

        {
            let mut data = self.data.lock().unwrap();
            data.canceler = Some(handle);
        }

        let runnable = self.runnable.clone();
        let data = self.data.clone();
        let task_store = {
            let data = data.lock().unwrap();
            data.task_store.clone()
        };
        let runnable_handle = async_std::task::spawn(async move {
            let _: BuckyResult<()> = async move {
                {
                    let mut tmp_data = data.lock().unwrap();
                    tmp_data.task_status = TaskStatus::Running;
                    runnable.status_change(tmp_data.task_status);
                }
                if task_store.is_some() {
                    task_store.as_ref().unwrap().save_task_status(&task_id, TaskStatus::Running).await?;
                }
                match ft.await {
                    Ok(ret) => {
                        match ret {
                            Ok(_) => {
                                {
                                    let mut tmp_data = data.lock().unwrap();
                                    tmp_data.task_status = TaskStatus::Finished;
                                    tmp_data.canceler = None;
                                    tmp_data.runnable_handle = None;
                                    runnable.status_change(tmp_data.task_status);
                                }
                                if task_store.is_some() {
                                    task_store.as_ref().unwrap().save_task_status(&task_id, TaskStatus::Finished).await?;
                                }
                            }
                            Err(err) => {
                                log::error!("task {} err {}", task_id.to_string(), err);
                                {
                                    let mut tmp_data = data.lock().unwrap();
                                    tmp_data.task_status = TaskStatus::Failed;
                                    tmp_data.canceler = None;
                                    tmp_data.runnable_handle = None;
                                    runnable.status_change(tmp_data.task_status);
                                }
                                if task_store.is_some() {
                                    task_store.as_ref().unwrap().save_task_status(&task_id, TaskStatus::Failed).await?;
                                }
                            }
                        }
                    }
                    Err(_) => {
                        {
                            let mut tmp_data = data.lock().unwrap();
                            tmp_data.task_status = TaskStatus::Stopped;
                            runnable.status_change(tmp_data.task_status);
                        }
                        if task_store.is_some() {
                            task_store.as_ref().unwrap().save_task_status(&task_id, TaskStatus::Stopped).await?;
                        }
                    }
                }
                Ok(())
            }.await;
        });
        {
            let mut data = self.data.lock().unwrap();
            data.runnable_handle = Some(runnable_handle);
        }
        Ok(())
    }

    async fn pause_task(&self) -> BuckyResult<()> {
        self.stop_task().await
    }

    async fn stop_task(&self) -> BuckyResult<()>
    {
        let (canceler, runnable_handle) = {
            let mut data = self.data.lock().unwrap();
            (data.canceler.take(), data.runnable_handle.take())
        };
        if canceler.is_some() {
            canceler.unwrap().abort();
            if runnable_handle.is_some() {
                runnable_handle.unwrap().await;
            }
            Ok(())
        } else {
            let err = format!("task [{}] is not running!", self.runnable.get_task_id());
            log::error!("{}", err);
            Err(BuckyError::from((BuckyErrorCode::ErrorState, err)))
        }
    }

    async fn check_and_waiting_stop(&self) {
        let (runnable_handle, waiting) = {
            let mut waiting_list = self.waiting_list.lock().unwrap();
            let mut data = self.data.lock().unwrap();
            let handle = data.runnable_handle.take();
            if handle.is_some() {
                (handle, None)
            } else {
                if data.task_status != TaskStatus::Running {
                    return;
                }
                let waiting = AsyncCondvar::new();
                waiting_list.push(waiting.clone());
                (None, Some(waiting))
            }
        };

        if runnable_handle.is_some() {
            runnable_handle.unwrap().await;
            let mut waiting_list = self.waiting_list.lock().unwrap();
            for waiting in waiting_list.iter() {
                waiting.notify();
            }
            waiting_list.clear();
        } else {
            waiting.unwrap().wait().await;
        }
    }

    async fn get_task_detail_status(&self) -> BuckyResult<Vec<u8>> {
        self.runnable.get_task_detail_status().await
    }
}

#[async_trait::async_trait]
pub trait TaskFactory: 'static + Send + Sync {
    fn get_task_type(&self) -> TaskType;
    async fn create(&self, params: &[u8]) -> BuckyResult<Box<dyn Task>>;
    async fn restore(&self, task_status: TaskStatus, params: &[u8], data: &[u8]) -> BuckyResult<Box<dyn Task>>;
}

#[cfg(test)]
mod test_task {
    use std::sync::Arc;
    use std::time::Duration;
    use cyfs_base::BuckyResult;
    use crate::{Runnable, RunnableTask, Task, TaskCategory, TaskId, TaskStatus, TaskStore, TaskType};

    struct TestRunnable {

    }

    #[async_trait::async_trait]
    impl Runnable for TestRunnable {
        fn get_task_id(&self) -> TaskId {
            TaskId::default()
        }

        fn get_task_type(&self) -> TaskType {
            todo!()
        }

        fn get_task_category(&self) -> TaskCategory {
            todo!()
        }

        async fn set_task_store(&mut self, _task_store: Arc<dyn TaskStore>) {
            todo!()
        }

        async fn run(&self) -> BuckyResult<()> {
            async_std::task::sleep(Duration::from_secs(10)).await;
            Ok(())
        }

        async fn get_task_detail_status(&self) -> BuckyResult<Vec<u8>> {
            todo!()
        }
    }
    #[test]
    fn test_runnable() {
        async_std::task::block_on(async {
            let task = RunnableTask::new(TestRunnable {});
            task.start_task().await.unwrap();
            async_std::task::sleep(Duration::from_secs(2)).await;
            assert_eq!(task.get_task_status().await, TaskStatus::Running);
            task.stop_task().await.unwrap();
            assert_eq!(task.get_task_status().await, TaskStatus::Stopped);
        });
    }
}

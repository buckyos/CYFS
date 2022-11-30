use crate::state::*;
use cyfs_base::*;

use std::thread::JoinHandle;
use thread_priority::ThreadBuilderExt;
use thread_priority::*;

pub struct PowRunner {
    manager: PoWThreadStateSyncRef,
}

impl PowRunner {
    pub fn new(manager: PoWThreadStateSyncRef) -> Self {
        Self { manager }
    }

    pub fn start(&self, difficulty: u8, count: u32) -> BuckyResult<Vec<JoinHandle<i32>>> {
        let mut threads = vec![];
        for i in 0..count {
            let thread = Self::start_thread(difficulty, i, self.manager.clone())?;

            threads.push(thread);
        }

        Ok(threads)
    }

    pub fn run(&self, difficulty: u8, count: u32) -> BuckyResult<()> {
        let threads = self.start(difficulty, count)?;
        for child in threads {
            // Wait for the thread to finish. Returns a result.
            let _ = child.join();
        }

        Ok(())
    }

    fn start_thread(
        difficulty: u8,
        index: u32,
        sync: PoWThreadStateSyncRef,
    ) -> BuckyResult<JoinHandle<i32>> {
        let name = format!("cyfs-pow-{}-{}", difficulty, index);

        let thread = std::thread::Builder::new()
            .name(name)
            .spawn_with_priority(ThreadPriority::Min, move|result| {
                if let Err(e) = result {
                    warn!("set pow thread priority failed! {:?}", e);
                }

                loop {
                    if let Some(state) = sync.select() {
                        if !Self::pow(state, sync.clone()) {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                0
            })
            .map_err(|e| {
                let msg = format!("start pow thread failed! {}", e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::Failed, msg)
            })?;

        Ok(thread)
    }

    fn pow(mut state: PoWThreadState, sync: PoWThreadStateSyncRef) -> bool {
        let mut current = state.range.start;
        let mut count: u32 = 0;
        let mut out_of_range = false;
        loop {
            if current >= state.range.end {
                out_of_range = true;
                break;
            }

            current += 1;

            let (diff, _hash) =
                ObjectDifficulty::difficulty(&state.data.object_id.as_slice(), &current);
            if diff >= state.data.difficulty {
                state.data.nonce = Some(current);
                state.data.difficulty = diff;
                break;
            }

            count += 1;
            if count > 1000 * 1000 * 10 {
                state.range.end = current;
                if !sync.sync(&state, PowThreadStatus::Sync) {
                    break;
                }
            }
        }

        state.range.end = current;
        sync.sync(&state, PowThreadStatus::Finished);

        out_of_range
    }
}

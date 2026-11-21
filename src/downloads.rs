use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Downloads {
    tasks: Mutex<VecDeque<Value>>,
    paused: AtomicBool,
    offline: AtomicBool,
    rate: AtomicU64,
    budget: Mutex<Option<Instant>>,
}
pub struct Transfer {
    owner: Arc<Downloads>,
    id: String,
    started: Instant,
    finished: bool,
}
impl Downloads {
    pub fn configure(&self, settings: &Value) {
        self.offline.store(
            settings["offlineMode"].as_bool().unwrap_or(false),
            Ordering::SeqCst,
        );
        let rate = settings["downloadLimitKiB"]
            .as_u64()
            .unwrap_or(0)
            .min(102400)
            * 1024;
        if self.rate.swap(rate, Ordering::SeqCst) != rate {
            *self.budget.lock().unwrap() = None;
        }
    }
    pub fn offline(&self) -> bool {
        self.offline.load(Ordering::SeqCst)
    }
    pub fn online(&self) -> Result<()> {
        if self.offline() {
            bail!("Включён режим без интернета. Для загрузки файлов отключите его в настройках.");
        }
        Ok(())
    }
    pub fn pause(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
    }
    pub fn check(&self) -> Result<()> {
        loop {
            self.online()?;
            if !self.paused.load(Ordering::SeqCst) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    pub fn status(&self) -> Value {
        let paused = self.paused.load(Ordering::SeqCst);
        let tasks: Vec<Value> = self
            .tasks
            .lock()
            .unwrap()
            .iter()
            .map(|task| {
                let mut task = task.clone();
                if paused
                    && ["connecting", "downloading"].contains(&task["phase"].as_str().unwrap_or(""))
                {
                    task["phase"] = json!("paused");
                    task["speed"] = json!(0);
                    task["eta"] = Value::Null;
                }
                task
            })
            .collect();
        json!({"paused":paused,"tasks":tasks})
    }
    pub fn clear(&self) {
        self.tasks
            .lock()
            .unwrap()
            .retain(|t| !["completed", "failed"].contains(&t["phase"].as_str().unwrap_or("")));
    }
    pub fn begin(self: &Arc<Self>, name: &str, total: u64) -> Transfer {
        let id = uuid::Uuid::new_v4().to_string();
        let mut tasks = self.tasks.lock().unwrap();
        if tasks.len() >= 100 {
            if let Some(index) = tasks
                .iter()
                .rposition(|t| ["completed", "failed"].contains(&t["phase"].as_str().unwrap_or("")))
            {
                tasks.remove(index);
            }
        }
        tasks.push_front(json!({"id":id,"name":name,"phase":"connecting","received":0,"total":total,"speed":0,"eta":null,"error":""}));
        Transfer {
            owner: self.clone(),
            id,
            started: Instant::now(),
            finished: false,
        }
    }
    // All eight asset workers use the same bandwidth budget, not eight independent limits.
    fn reserve(&self, bytes: usize) -> Result<()> {
        self.check()?;
        let rate = self.rate.load(Ordering::SeqCst);
        if rate == 0 {
            return Ok(());
        }
        let deadline = {
            let mut budget = self.budget.lock().unwrap();
            let now = Instant::now();
            let deadline = budget.unwrap_or(now).max(now)
                + Duration::from_secs_f64(bytes as f64 / rate as f64);
            *budget = Some(deadline);
            deadline
        };
        while Instant::now() < deadline {
            self.check()?;
            if self.rate.load(Ordering::SeqCst) != rate {
                break;
            }
            std::thread::sleep(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(50)),
            );
        }
        Ok(())
    }
}
impl Transfer {
    fn update(&self, change: impl FnOnce(&mut Value)) {
        if let Some(task) = self
            .owner
            .tasks
            .lock()
            .unwrap()
            .iter_mut()
            .find(|t| t["id"] == self.id)
        {
            change(task);
        }
    }
    pub fn check(&self) -> Result<()> {
        self.owner.check()
    }
    pub fn throttle(&self, bytes: usize) -> Result<()> {
        self.owner.reserve(bytes)
    }
    pub fn progress(&self, received: u64, total: u64) {
        let speed = (received as f64 / self.started.elapsed().as_secs_f64().max(0.1)) as u64;
        self.update(|t| {
            t["received"] = json!(received);
            t["total"] = json!(total);
            t["speed"] = json!(speed);
            t["phase"] = json!("downloading");
            t["eta"] = if speed > 0 && total > received {
                json!((total - received) / speed)
            } else {
                Value::Null
            };
        });
    }
    pub fn finish(&mut self, error: Option<&str>) {
        self.update(|t| {
            t["phase"] = json!(if error.is_some() {
                "failed"
            } else {
                "completed"
            });
            t["speed"] = json!(0);
            t["error"] = json!(error.unwrap_or(""));
        });
        self.finished = true;
    }
}
impl Drop for Transfer {
    fn drop(&mut self) {
        if !self.finished {
            self.finish(Some("Загрузка прервана"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pause_and_terminal_state_survive_progress_and_clear() {
        let manager = Arc::new(Downloads::default());
        let mut task = manager.begin("client.jar", 100);
        task.progress(25, 100);
        manager.pause(true);
        assert_eq!(manager.status()["tasks"][0]["phase"], "paused");
        manager.clear();
        assert_eq!(manager.status()["tasks"].as_array().unwrap().len(), 1);
        manager.pause(false);
        task.finish(None);
        assert_eq!(manager.status()["tasks"][0]["phase"], "completed");
        manager.clear();
        assert!(manager.status()["tasks"].as_array().unwrap().is_empty());
    }
    #[test]
    fn shared_limit_and_offline_interrupt_waiting_workers() {
        let manager = Arc::new(Downloads::default());
        manager.configure(&json!({"downloadLimitKiB":1}));
        let started = Instant::now();
        manager.reserve(128).unwrap();
        manager.reserve(128).unwrap();
        assert!(started.elapsed() >= Duration::from_millis(240));
        manager.pause(true);
        let worker = manager.clone();
        let thread = std::thread::spawn(move || worker.check());
        manager.configure(&json!({"offlineMode":true}));
        assert!(thread.join().unwrap().is_err());
    }
}

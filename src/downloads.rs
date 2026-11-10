use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex, Weak,
    },
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Downloads {
    tasks: Mutex<VecDeque<Task>>,
    paths: Mutex<HashMap<std::path::PathBuf, Weak<Mutex<()>>>>,
    cancelled: AtomicBool,
}
struct Task {
    id: String,
    name: String,
    phase: String,
    received: u64,
    total: u64,
    speed: u64,
    attempt: u32,
    error: String,
    control: Arc<AtomicU8>,
}
pub struct Transfer<'a> {
    owner: &'a Downloads,
    pub id: String,
    control: Arc<AtomicU8>,
    started: Instant,
}
impl Downloads {
    pub fn destination(&self, path: &std::path::Path) -> Arc<Mutex<()>> {
        let mut paths = self.paths.lock().unwrap();
        paths.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = paths.get(path).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(Mutex::new(()));
        paths.insert(path.to_path_buf(), Arc::downgrade(&lock));
        lock
    }
    pub fn begin(&self, name: &str, total: u64) -> Transfer<'_> {
        let id = uuid::Uuid::new_v4().to_string();
        let control = Arc::new(AtomicU8::new(0));
        let mut tasks = self.tasks.lock().unwrap();
        if tasks.len() >= 100 {
            if let Some(index) = tasks
                .iter()
                .position(|t| t.control.load(Ordering::SeqCst) == 3)
            {
                tasks.remove(index);
            }
        }
        tasks.push_front(Task {
            id: id.clone(),
            name: name.into(),
            phase: "queued".into(),
            received: 0,
            total,
            speed: 0,
            attempt: 0,
            error: String::new(),
            control: control.clone(),
        });
        Transfer {
            owner: self,
            id,
            control,
            started: Instant::now(),
        }
    }
    pub fn status(&self) -> Value {
        json!(self.tasks.lock().unwrap().iter().map(|t|json!({"id":t.id,"name":t.name,"phase":match t.control.load(Ordering::SeqCst){1=>"paused",2=>"cancelling",_=>t.phase.as_str()},"received":t.received,"total":t.total,"speed":t.speed,"attempt":t.attempt,"error":t.error,"eta":if t.speed>0&&t.total>t.received {Some((t.total-t.received)/t.speed)}else{None}})).collect::<Vec<_>>())
    }
    pub fn control(&self, id: &str, action: &str) -> Result<Value> {
        let mut tasks = self.tasks.lock().unwrap();
        let task = tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| anyhow::anyhow!("Загрузка не найдена"))?;
        if task.control.load(Ordering::SeqCst) == 3 {
            bail!("Загрузка уже завершена");
        }
        match action {
            "pause" => {
                task.control.store(1, Ordering::SeqCst);
                task.phase = "paused".into();
                task.speed = 0;
            }
            "resume" => {
                task.control.store(0, Ordering::SeqCst);
                task.phase = "queued".into();
            }
            "cancel" => {
                self.cancelled.store(true, Ordering::SeqCst);
                for task in tasks
                    .iter_mut()
                    .filter(|t| t.control.load(Ordering::SeqCst) != 3)
                {
                    task.control.store(2, Ordering::SeqCst);
                    task.phase = "cancelling".into();
                }
            }
            _ => bail!("Неизвестное действие загрузки"),
        }
        Ok(Value::Null)
    }
    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }
    pub fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::SeqCst) {
            bail!("Загрузка отменена. Повторите исходное действие, чтобы продолжить.");
        }
        Ok(())
    }
    pub fn clear(&self) {
        self.tasks
            .lock()
            .unwrap()
            .retain(|t| t.control.load(Ordering::SeqCst) != 3);
    }
}
impl Transfer<'_> {
    fn update(&self, f: impl FnOnce(&mut Task)) {
        if let Some(task) = self
            .owner
            .tasks
            .lock()
            .unwrap()
            .iter_mut()
            .find(|t| t.id == self.id)
        {
            f(task);
        }
    }
    pub fn check(&self) -> Result<()> {
        loop {
            match self.control.load(Ordering::SeqCst) {
                2 => bail!("Загрузка отменена. Повторите исходное действие, чтобы продолжить."),
                1 => std::thread::sleep(Duration::from_millis(80)),
                _ => return Ok(()),
            }
        }
    }
    pub fn attempt(&mut self, attempt: u32) {
        self.started = Instant::now();
        self.update(|t| {
            t.attempt = attempt;
            t.phase = "connecting".into();
            t.error.clear();
            t.speed = 0;
        });
    }
    pub fn progress(&self, received: u64, total: u64, fresh: u64) {
        let speed = (fresh as f64 / self.started.elapsed().as_secs_f64().max(0.1)) as u64;
        self.update(|t| {
            t.received = received;
            if total > 0 {
                t.total = total;
            }
            t.speed = speed;
            t.phase = "downloading".into();
        });
    }
    pub fn phase(&self, phase: &str) {
        self.update(|t| {
            t.phase = phase.into();
            t.speed = 0;
        });
    }
    pub fn finish(&self, error: Option<&str>) {
        let cancelled = self.control.load(Ordering::SeqCst) == 2;
        self.update(|t| {
            t.phase = if cancelled {
                "cancelled"
            } else if error.is_some() {
                "failed"
            } else {
                "completed"
            }
            .into();
            t.error = error.unwrap_or("").into();
            t.speed = 0;
        });
        self.control.store(3, Ordering::SeqCst);
    }
    pub fn cancelled(&self) -> bool {
        self.control.load(Ordering::SeqCst) == 2
    }
}
impl Drop for Transfer<'_> {
    fn drop(&mut self) {
        if self.control.load(Ordering::SeqCst) != 3 {
            self.finish(Some("Загрузка прервана"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pause_resume_cancel_and_clear_have_terminal_states() {
        let manager = Downloads::default();
        let task = manager.begin("mod.jar", 42);
        manager.control(&task.id, "pause").unwrap();
        task.progress(10, 42, 10);
        assert_eq!(manager.status()[0]["phase"], "paused");
        manager.control(&task.id, "resume").unwrap();
        assert!(task.check().is_ok());
        manager.control(&task.id, "cancel").unwrap();
        assert!(task.check().is_err());
        task.finish(Some("cancelled"));
        assert_eq!(manager.status()[0]["phase"], "cancelled");
        assert!(manager.control(&task.id, "resume").is_err());
        assert!(manager.check().is_err());
        manager.reset();
        assert!(manager.check().is_ok());
        manager.clear();
        assert_eq!(manager.status(), json!([]));
    }
}

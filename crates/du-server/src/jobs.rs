//! Очередь задач: ОДИН воркер прогоняет всё, что трогает видеокарту, строго по очереди.
//!
//! На машине одна карта, а моделей четыре — нарратор, картинка, озвучка и распознавание.
//! Раньше очерёдность держал клиент, и стоило двум задачам совпасть, как обе модели
//! оказывались в памяти разом и карта вставала. Теперь порядок задаёт сервер, и обойти его
//! клиент не может.
//!
//! Прогресс уходит по SSE, результат — через одноразовый канал. Завершённая задача убирается
//! по таймеру, даже если за результатом так и не пришли.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};

/// Тело задачи: получает колбэк прогресса, возвращает результат.
pub type JobFn = Box<dyn FnOnce(ProgressFn) -> Result<Value, String> + Send + 'static>;

/// Колбэк прогресса, который тело задачи зовёт по ходу работы.
pub type ProgressFn = Arc<dyn Fn(Value) + Send + Sync + 'static>;

/// Сколько держать завершённую задачу, если за результатом не пришли.
const REAP_AFTER: Duration = Duration::from_secs(300);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum JobStatus {
    Queued,
    Running,
    Done,
    Error,
}

struct Job {
    events: broadcast::Sender<Value>,
    status: JobStatus,
    result: Option<Value>,
    error: Option<String>,
    abandoned: bool,
    result_sender: Option<oneshot::Sender<Result<Value, String>>>,
}

#[derive(Clone)]
pub struct JobQueue {
    jobs: Arc<Mutex<HashMap<String, Job>>>,
    submit: mpsc::UnboundedSender<(String, JobFn)>,
}

fn new_job_id() -> String {
    uuid::Uuid::new_v4().simple().to_string().chars().take(12).collect()
}

impl JobQueue {
    pub fn new() -> Self {
        let jobs: Arc<Mutex<HashMap<String, Job>>> = Arc::new(Mutex::new(HashMap::new()));
        let (submit, mut incoming) = mpsc::unbounded_channel::<(String, JobFn)>();

        let worker_jobs = jobs.clone();
        tokio::spawn(async move {
            while let Some((id, body)) = incoming.recv().await {
                // Клиент мог уйти, пока задача ждала очереди — тогда работу не начинаем.
                let events = {
                    let mut map = worker_jobs.lock().await;
                    match map.get_mut(&id) {
                        None => continue,
                        Some(job) if job.abandoned => {
                            map.remove(&id);
                            continue;
                        }
                        Some(job) => {
                            job.status = JobStatus::Running;
                            job.events.clone()
                        }
                    }
                };

                let sink = events.clone();
                let progress: ProgressFn = Arc::new(move |event: Value| {
                    let mut fields = match event {
                        Value::Object(map) => map,
                        other => {
                            let mut map = serde_json::Map::new();
                            map.insert("msg".into(), other);
                            map
                        }
                    };
                    fields.insert("type".into(), json!("progress"));
                    // Без подписчиков отправка не удаётся — это нормально, задача идёт дальше.
                    let _ = sink.send(Value::Object(fields));
                });

                // Тело синхронное и тяжёлое, поэтому уезжает в блокирующий пул.
                let outcome = tokio::task::spawn_blocking(move || body(progress))
                    .await
                    .unwrap_or_else(|error| Err(format!("задача упала: {error}")));

                {
                    let mut map = worker_jobs.lock().await;
                    if let Some(job) = map.get_mut(&id) {
                        match &outcome {
                            Ok(value) => {
                                job.status = JobStatus::Done;
                                job.result = Some(value.clone());
                                let _ = events.send(json!({ "type": "done", "result": value }));
                            }
                            Err(error) => {
                                job.status = JobStatus::Error;
                                job.error = Some(error.clone());
                                let _ = events.send(json!({ "type": "error", "error": error }));
                            }
                        }
                        if let Some(sender) = job.result_sender.take() {
                            let _ = sender.send(outcome);
                        }
                    }
                }

                let reap_jobs = worker_jobs.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(REAP_AFTER).await;
                    reap_jobs.lock().await.remove(&id);
                });
            }
        });

        JobQueue { jobs, submit }
    }

    fn make_job(&self, result_sender: Option<oneshot::Sender<Result<Value, String>>>) -> (String, Job) {
        let (events, _) = broadcast::channel(256);
        (
            new_job_id(),
            Job {
                events,
                status: JobStatus::Queued,
                result: None,
                error: None,
                abandoned: false,
                result_sender,
            },
        )
    }

    /// Поставить задачу в очередь и вернуть её идентификатор.
    pub async fn enqueue(&self, body: JobFn) -> String {
        let (id, job) = self.make_job(None);
        self.jobs.lock().await.insert(id.clone(), job);
        let _ = self.submit.send((id.clone(), body));
        id
    }

    /// Поставить задачу и получить канал результата — когда ответ нужен прямо в этом запросе.
    pub async fn enqueue_awaitable(
        &self,
        body: JobFn,
    ) -> (String, oneshot::Receiver<Result<Value, String>>) {
        let (sender, receiver) = oneshot::channel();
        let (id, job) = self.make_job(Some(sender));
        self.jobs.lock().await.insert(id.clone(), job);
        let _ = self.submit.send((id.clone(), body));
        (id, receiver)
    }

    /// Подписаться на события задачи. Вторым значением — финальное событие, если задача уже
    /// завершилась: подписка могла опоздать, и без этого клиент завис бы навсегда.
    pub async fn subscribe(&self, id: &str) -> Option<(broadcast::Receiver<Value>, Option<Value>)> {
        let map = self.jobs.lock().await;
        let job = map.get(id)?;
        let receiver = job.events.subscribe();
        let terminal = match job.status {
            JobStatus::Done => Some(json!({ "type": "done", "result": job.result })),
            JobStatus::Error => {
                Some(json!({ "type": "error", "error": job.error.clone().unwrap_or_default() }))
            }
            _ => None,
        };
        Some((receiver, terminal))
    }

    /// Пометить, что результат больше не нужен: если задача ещё не начата, её не запустят.
    pub async fn abandon(&self, id: &str) {
        if let Some(job) = self.jobs.lock().await.get_mut(id) {
            job.abandoned = true;
        }
    }

    pub async fn status(&self, id: &str) -> Option<JobStatus> {
        self.jobs.lock().await.get(id).map(|job| job.status)
    }
}

impl Default for JobQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_job_runs_and_delivers_its_result() {
        let queue = JobQueue::new();
        let (_, result) = queue
            .enqueue_awaitable(Box::new(|progress| {
                progress(json!({ "msg": "работаю" }));
                Ok(json!({ "готово": true }))
            }))
            .await;
        let value = result.await.unwrap().unwrap();
        assert_eq!(value["готово"], true);
    }

    #[tokio::test]
    async fn a_failing_job_reports_its_error() {
        let queue = JobQueue::new();
        let (_, result) = queue
            .enqueue_awaitable(Box::new(|_| Err("не вышло".to_string())))
            .await;
        assert_eq!(result.await.unwrap().unwrap_err(), "не вышло");
    }

    #[tokio::test]
    async fn a_panicking_job_does_not_take_the_queue_down() {
        let queue = JobQueue::new();
        let (_, first) = queue
            .enqueue_awaitable(Box::new(|_| panic!("внутренняя ошибка")))
            .await;
        assert!(first.await.unwrap().is_err());

        // Очередь обязана продолжить работать после падения предыдущей задачи.
        let (_, second) = queue.enqueue_awaitable(Box::new(|_| Ok(json!(2)))).await;
        assert_eq!(second.await.unwrap().unwrap(), 2);
    }

    #[tokio::test]
    async fn jobs_run_strictly_one_after_another() {
        let queue = JobQueue::new();
        let running = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let peak = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut waiters = Vec::new();

        for _ in 0..6 {
            let running = running.clone();
            let peak = peak.clone();
            let (_, result) = queue
                .enqueue_awaitable(Box::new(move |_| {
                    use std::sync::atomic::Ordering;
                    let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(20));
                    running.fetch_sub(1, Ordering::SeqCst);
                    Ok(json!(null))
                }))
                .await;
            waiters.push(result);
        }
        for waiter in waiters {
            waiter.await.unwrap().unwrap();
        }
        assert_eq!(
            peak.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "две задачи одновременно означают две модели на карте — этого быть не должно"
        );
    }

    #[tokio::test]
    async fn an_abandoned_job_never_starts() {
        let queue = JobQueue::new();
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));

        // Занимаем воркер, чтобы следующая задача точно постояла в очереди.
        let (_, blocker) = queue
            .enqueue_awaitable(Box::new(|_| {
                std::thread::sleep(Duration::from_millis(120));
                Ok(json!(null))
            }))
            .await;

        let flag = started.clone();
        let id = queue
            .enqueue(Box::new(move |_| {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(json!(null))
            }))
            .await;
        queue.abandon(&id).await;

        blocker.await.unwrap().unwrap();
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(!started.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn a_late_subscriber_still_gets_the_final_event() {
        let queue = JobQueue::new();
        let (id, result) = queue.enqueue_awaitable(Box::new(|_| Ok(json!("всё")))).await;
        result.await.unwrap().unwrap();

        let (_, terminal) = queue.subscribe(&id).await.expect("задача ещё не убрана");
        let terminal = terminal.expect("опоздавшему обязаны отдать финальное событие");
        assert_eq!(terminal["type"], "done");
        assert_eq!(terminal["result"], "всё");
    }
}

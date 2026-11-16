//! 将控制器和音频设备限制在同一个线程，通过消息串行处理命令和设备事件。

use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver, RecvTimeoutError, Sender},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use music_audio::EngineEvent;

use crate::controller::{AudioBackend, Controller, ControllerEvent, PlayerState};

#[derive(Debug, Clone)]
pub enum Projection {
    State(PlayerState),
    Position { ms: u64 },
    Notice { title: String },
    DeviceError(String),
}

type Action<B> = Box<dyn FnOnce(&mut Controller<B>) + Send>;
enum Message<B: AudioBackend> {
    Execute(Action<B>),
    Audio(u64, EngineEvent),
    Shutdown,
}

pub struct Actor<B: AudioBackend> {
    sender: Sender<Message<B>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl<B: AudioBackend + 'static> Actor<B> {
    /// 工厂在工作线程创建设备，设备类型无需实现 Send。
    pub fn spawn<F, E>(
        session: PathBuf,
        resolver: crate::controller::LocationResolver,
        factory: F,
        emit: E,
    ) -> Result<Self, String>
    where
        F: FnOnce(Box<dyn Fn(EngineEvent) + Send>) -> Result<B, String> + Send + 'static,
        E: Fn(Projection) + Send + 'static,
    {
        let (sender, receiver) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let generation = Arc::new(AtomicU64::new(0));
        let audio_generation = Arc::clone(&generation);
        let audio_sender = sender.clone();
        let worker = thread::spawn(move || {
            let callback = Box::new(move |event| {
                let generation = audio_generation.load(Ordering::SeqCst);
                let _ = audio_sender.send(Message::Audio(generation, event));
            });
            let mut controller = match factory(callback).and_then(|backend| {
                Controller::new(session, backend, Arc::clone(&generation), resolver).map_err(|e| e.to_string())
            }) {
                Ok(controller) => controller,
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };
            if ready_tx.send(Ok(())).is_ok() {
                run(&mut controller, receiver, &generation, emit);
            }
        });
        match ready_rx.recv().map_err(|_| "播放器线程启动失败".to_string())? {
            Ok(()) => Ok(Self {
                sender,
                worker: Mutex::new(Some(worker)),
            }),
            Err(error) => {
                let _ = worker.join();
                Err(error)
            }
        }
    }

    /// 在控制器线程执行命令。返回值经通道交还调用方。
    pub fn call<R, F>(&self, action: F) -> Result<R, String>
    where
        R: Send + 'static,
        F: FnOnce(&mut Controller<B>) -> R + Send + 'static,
    {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .send(Message::Execute(Box::new(move |controller| {
                let _ = sender.send(action(controller));
            })))
            .map_err(|_| "播放器线程已停止".to_string())?;
        receiver.recv().map_err(|_| "播放器线程已停止".to_string())
    }

    pub fn shutdown(&self) {
        let _ = self.sender.send(Message::Shutdown);
        if let Some(worker) = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = worker.join();
        }
    }
}

impl<B: AudioBackend> Drop for Actor<B> {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Shutdown);
        if let Some(worker) = self.worker.get_mut().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = worker.join();
        }
    }
}

fn run<B: AudioBackend>(
    controller: &mut Controller<B>,
    receiver: Receiver<Message<B>>,
    generation: &AtomicU64,
    emit: impl Fn(Projection),
) {
    let mut last_position = Instant::now();
    loop {
        match receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(Message::Execute(action)) => action(controller),
            Ok(Message::Audio(id, event)) if id == generation.load(Ordering::SeqCst) => match event {
                EngineEvent::Ended => controller.on_ended(),
                EngineEvent::Error(error) => controller.on_device_error(error),
            },
            Ok(Message::Audio(..)) => {}
            Ok(Message::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                if let Err(error) = controller.save() {
                    eprintln!("保存播放会话失败：{error}");
                }
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        match controller.poll() {
            Ok(events) => {
                for event in events {
                    match event {
                        ControllerEvent::StateChanged => emit(Projection::State(controller.state())),
                        ControllerEvent::Notice { title } => emit(Projection::Notice { title }),
                        ControllerEvent::DeviceError(error) => emit(Projection::DeviceError(error)),
                    }
                }
            }
            Err(error) => eprintln!("更新播放会话失败：{error}"),
        }
        if last_position.elapsed() >= Duration::from_millis(200) {
            let state = controller.state();
            if state.playing {
                emit(Projection::Position { ms: state.position_ms });
            }
            last_position = Instant::now();
        }
    }
}

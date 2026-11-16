//! 输出设备不可用时保留曲库能力，并向播放命令报告错误；设备丢失后可按当前默认设备重开。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use music_audio::{DeviceOutput, EngineEvent};

use crate::controller::{AudioBackend, LocationResolver};

#[derive(Clone, Default)]
pub struct AudioAvailability(Arc<Mutex<Option<String>>>);

impl AudioAvailability {
    pub fn set_error(&self, error: Option<String>) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = error;
    }

    pub fn require(&self) -> Result<(), String> {
        match &*self.0.lock().unwrap_or_else(|e| e.into_inner()) {
            Some(error) => Err(format!("音频输出不可用：{error}")),
            None => Ok(()),
        }
    }
}

type SharedCallback = Arc<Mutex<Box<dyn FnMut(EngineEvent) + Send>>>;

/// 每次打开设备都要一个新的回调；转发到同一个上层回调，设备重开后事件仍送往控制器。
fn forward(callback: &SharedCallback) -> impl FnMut(EngineEvent) + Send + 'static {
    let callback = Arc::clone(callback);
    move |event| (callback.lock().unwrap_or_else(|e| e.into_inner()))(event)
}

pub struct DeviceBackend {
    output: Option<DeviceOutput>,
    callback: SharedCallback,
    error: Option<String>,
    availability: AudioAvailability,
    resolver: LocationResolver,
}

impl DeviceBackend {
    pub fn open(
        callback: impl FnMut(EngineEvent) + Send + 'static,
        availability: AudioAvailability,
        resolver: LocationResolver,
    ) -> Self {
        let callback: SharedCallback = Arc::new(Mutex::new(Box::new(callback)));
        match DeviceOutput::open(forward(&callback)) {
            Ok(output) => Self {
                output: Some(output),
                callback,
                error: None,
                availability,
                resolver,
            },
            Err(error) => {
                availability.set_error(Some(error.clone()));
                Self {
                    output: None,
                    callback,
                    error: Some(error),
                    availability,
                    resolver,
                }
            }
        }
    }

    pub fn unavailable(error: String) -> Self {
        let availability = AudioAvailability::default();
        availability.set_error(Some(error.clone()));
        Self {
            output: None,
            callback: Arc::new(Mutex::new(Box::new(|_| {}))),
            error: Some(error),
            availability,
            resolver: Arc::new(|location| {
                let path = std::path::PathBuf::from(location);
                let ext = path.extension().and_then(|ext| ext.to_str()).map(str::to_owned);
                Ok((std::fs::File::open(&path)?, ext))
            }),
        }
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

impl AudioBackend for DeviceBackend {
    fn available(&self) -> bool {
        self.output.is_some() && self.availability.require().is_ok()
    }
    fn recover(&mut self) -> bool {
        // 先关闭失效的流，再按当前默认设备重开（设备可能已换成扬声器或新的耳机）
        self.output = None;
        match DeviceOutput::open(forward(&self.callback)) {
            Ok(output) => {
                self.output = Some(output);
                self.error = None;
                self.availability.set_error(None);
                true
            }
            Err(error) => {
                self.availability.set_error(Some(error.clone()));
                self.error = Some(error);
                false
            }
        }
    }
    fn load(&mut self, location: &str) -> Result<Duration, String> {
        self.availability.require()?;
        let (file, ext) = (self.resolver)(location).map_err(|e| e.to_string())?;
        match &self.output {
            Some(output) => output.load_file(file, ext.as_deref()).map_err(|e| e.to_string()),
            None => Err(self.error.clone().unwrap_or_else(|| "音频输出不可用".into())),
        }
    }
    fn play(&mut self) {
        if let Some(output) = &self.output {
            output.play();
        }
    }
    fn pause(&mut self) {
        if let Some(output) = &self.output {
            output.pause();
        }
    }
    fn seek(&mut self, position: Duration) -> Result<(), String> {
        self.availability.require()?;
        match &self.output {
            Some(output) => output.seek(position).map_err(|e| e.to_string()),
            None => Err(self.error.clone().unwrap_or_else(|| "音频输出不可用".into())),
        }
    }
    fn set_volume(&mut self, volume: f32) {
        if let Some(output) = &self.output {
            output.set_volume(volume);
        }
    }
    fn set_rate(&mut self, rate: f32) {
        if let Some(output) = &self.output {
            output.set_rate(rate);
        }
    }
    fn set_pitch(&mut self, semitones: i8) {
        if let Some(output) = &self.output {
            output.set_pitch(semitones);
        }
    }
    fn position(&self) -> Duration {
        self.output.as_ref().map_or(Duration::ZERO, DeviceOutput::position)
    }
}

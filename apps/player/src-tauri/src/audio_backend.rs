//! 输出设备不可用时保留曲库能力，并向播放命令报告错误。

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use music_audio::{DeviceOutput, EngineEvent};

use crate::controller::AudioBackend;

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

pub struct DeviceBackend {
    output: Option<DeviceOutput>,
    error: Option<String>,
    availability: AudioAvailability,
}

impl DeviceBackend {
    pub fn open(callback: impl FnMut(EngineEvent) + Send + 'static, availability: AudioAvailability) -> Self {
        match DeviceOutput::open(callback) {
            Ok(output) => Self {
                output: Some(output),
                error: None,
                availability,
            },
            Err(error) => {
                availability.set_error(Some(error.clone()));
                Self {
                    output: None,
                    error: Some(error),
                    availability,
                }
            }
        }
    }

    pub fn unavailable(error: String) -> Self {
        let availability = AudioAvailability::default();
        availability.set_error(Some(error.clone()));
        Self {
            output: None,
            error: Some(error),
            availability,
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
    fn load(&mut self, path: &Path) -> Result<Duration, String> {
        self.availability.require()?;
        match &self.output {
            Some(output) => output.load(path).map_err(|e| e.to_string()),
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

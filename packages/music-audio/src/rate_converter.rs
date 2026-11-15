//! 保持分块边界连续的线性插值采样率转换。

pub struct RateConverter {
    input_rate: u32,
    output_rate: u32,
    channels: usize,
    input: Vec<f32>,
    // 输入帧位置，以 output_rate 为分母，避免浮点误差随播放时间累积。
    position: u64,
    input_frames: u64,
    output_frames: u64,
}

impl RateConverter {
    pub fn new(input_rate: u32, output_rate: u32, channels: u16) -> Self {
        assert!(input_rate > 0 && output_rate > 0 && channels > 0);
        Self {
            input_rate,
            output_rate,
            channels: channels as usize,
            input: Vec::new(),
            position: 0,
            input_frames: 0,
            output_frames: 0,
        }
    }

    /// 接收交错采样，向输出追加已可插值的完整帧。
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        assert_eq!(input.len() % self.channels, 0);
        self.input_frames += (input.len() / self.channels) as u64;
        self.input.extend_from_slice(input);
        let frames = self.input.len() / self.channels;
        while self.position / self.output_rate as u64 + 1 < frames as u64 {
            let index = (self.position / self.output_rate as u64) as usize;
            let fraction = (self.position % self.output_rate as u64) as f32 / self.output_rate as f32;
            for channel in 0..self.channels {
                let a = self.input[index * self.channels + channel];
                let b = self.input[(index + 1) * self.channels + channel];
                output.push(a + (b - a) * fraction);
            }
            self.position += self.input_rate as u64;
            self.output_frames += 1;
        }
        let consumed = ((self.position / self.output_rate as u64) as usize).min(frames.saturating_sub(1));
        self.input.drain(..consumed * self.channels);
        self.position -= consumed as u64 * self.output_rate as u64;
    }

    /// 完成当前流，输出末尾帧并重置，以便复用转换器。
    pub fn flush(&mut self, output: &mut Vec<f32>) {
        let target = ((self.input_frames as u128 * self.output_rate as u128 + self.input_rate as u128 / 2)
            / self.input_rate as u128) as u64;
        if !self.input.is_empty() {
            let last = &self.input[self.input.len() - self.channels..];
            for _ in self.output_frames..target {
                output.extend_from_slice(last);
            }
        }
        self.input.clear();
        self.position = 0;
        self.input_frames = 0;
        self.output_frames = 0;
    }
}

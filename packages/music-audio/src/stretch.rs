//! 倍速与升降调：WSOLA 时间伸缩 + 线性插值重采样，思路与 SoundTouch 相同。
//!
//! 升降调 p 倍、倍速 r 时，先用 WSOLA 把时长拉成 p/r 倍（音高不变），再按 p 倍速度重采样，
//! 最终时长为 1/r、音高为 p 倍。

use crate::Pcm;

/// WSOLA 每段长度、段间交叠、对齐搜索范围（毫秒）。
const SEQUENCE_MS: u32 = 40;
const OVERLAP_MS: u32 = 10;
const SEEK_MS: u32 = 15;
/// 粗搜索步长（帧），之后在最佳点附近逐帧细搜。
const COARSE_STEP: usize = 8;

/// 时间伸缩处理器：倍速改变时长不改变音高，升降调改变音高不改变时长。
pub struct Stretcher {
    channels: usize,
    rate: f64,
    pitch: f64,

    sequence: usize,
    overlap: usize,
    seek: usize,
    /// 等待 WSOLA 处理的输入（交错）。
    input: Vec<f32>,
    /// 上一段留下、要与下一段交叠的尾部。
    tail: Vec<f32>,
    started: bool,
    skip_fraction: f64,

    /// 等待重采样的输入（交错）和读取位置（帧，含小数）。
    resample_input: Vec<f32>,
    resample_pos: f64,

    /// 按已输入帧数和当时的倍速应输出的总帧数，`flush` 时据此补齐或截断。
    expected_frames: f64,
    emitted_frames: u64,
}

impl Stretcher {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let frames = |ms: u32| (sample_rate as usize * ms as usize / 1000).max(1);
        Self {
            channels: channels.max(1) as usize,
            rate: 1.0,
            pitch: 1.0,
            sequence: frames(SEQUENCE_MS),
            overlap: frames(OVERLAP_MS),
            seek: frames(SEEK_MS),
            input: Vec::new(),
            tail: Vec::new(),
            started: false,
            skip_fraction: 0.0,
            resample_input: Vec::new(),
            resample_pos: 0.0,
            expected_frames: 0.0,
            emitted_frames: 0,
        }
    }

    pub fn set_rate(&mut self, rate: f32) {
        self.rate = if rate.is_finite() {
            (rate as f64).clamp(0.25, 4.0)
        } else {
            1.0
        };
    }

    pub fn set_pitch(&mut self, semitones: i8) {
        self.pitch = 2_f64.powf(semitones.clamp(-24, 24) as f64 / 12.0);
    }

    /// 输入一段交错采样，把可用的输出追加到 `output`。
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        let frames = input.len() / self.channels;
        self.expected_frames += frames as f64 / self.rate;

        // 正常倍速、不升降调且没有积压时直接透传，不损失音质
        if self.is_identity() && self.is_idle() {
            output.extend_from_slice(&input[..frames * self.channels]);
            self.emitted_frames += frames as u64;
            return;
        }

        self.input.extend_from_slice(&input[..frames * self.channels]);
        let mut stretched = Vec::new();
        self.run_wsola(&mut stretched);
        let before = output.len();
        self.resample(&stretched, output);
        self.emitted_frames += ((output.len() - before) / self.channels) as u64;
    }

    /// 输入结束，取出剩余输出；输出总长度与“输入帧数 ÷ 倍速”一致。之后可以接着处理新的音频。
    pub fn flush(&mut self, output: &mut Vec<f32>) {
        let ch = self.channels;
        let mut rest = Vec::new();
        if self.started || !self.input.is_empty() {
            // 用静音把积压的输入推出 WSOLA，多出的部分最后截掉
            let padding = self.required_frames() + self.sequence;
            self.input.resize(self.input.len() + padding * ch, 0.0);
            let mut stretched = Vec::new();
            self.run_wsola(&mut stretched);
            stretched.extend_from_slice(&self.tail);
            self.resample(&stretched, &mut rest);
        }
        if !self.resample_input.is_empty() {
            let silence = vec![0.0; 2 * ch];
            self.resample(&silence, &mut rest);
        }

        let target = self.expected_frames.round() as u64;
        let missing = target.saturating_sub(self.emitted_frames) as usize;
        let take = missing.min(rest.len() / ch);
        output.extend_from_slice(&rest[..take * ch]);
        output.resize(output.len() + (missing - take) * ch, 0.0);

        self.reset();
    }

    fn reset(&mut self) {
        self.input.clear();
        self.tail.clear();
        self.started = false;
        self.skip_fraction = 0.0;
        self.resample_input.clear();
        self.resample_pos = 0.0;
        self.expected_frames = 0.0;
        self.emitted_frames = 0;
    }

    fn is_identity(&self) -> bool {
        self.rate == 1.0 && self.pitch == 1.0
    }

    fn is_idle(&self) -> bool {
        !self.started && self.input.is_empty() && self.resample_input.is_empty()
    }

    /// WSOLA 的速度系数：每输出一帧消耗多少输入帧。
    fn tempo(&self) -> f64 {
        self.rate / self.pitch
    }

    fn nominal_skip(&self) -> f64 {
        self.tempo() * (self.sequence - self.overlap) as f64
    }

    /// 处理一段所需的最少输入帧数。
    fn required_frames(&self) -> usize {
        (self.nominal_skip().ceil() as usize + self.overlap).max(self.sequence) + self.seek
    }

    /// 每轮：在搜索范围内找与上一段尾部最相似的位置，交叉淡化后输出一段，再按速度前移。
    fn run_wsola(&mut self, output: &mut Vec<f32>) {
        let ch = self.channels;
        let (sequence, overlap) = (self.sequence, self.overlap);
        let middle = sequence - 2 * overlap;
        let required = self.required_frames();
        let nominal_skip = self.nominal_skip();

        while self.input.len() / ch >= required {
            let offset = if self.started {
                self.best_offset()
            } else {
                self.tail = self.input[..overlap * ch].to_vec();
                self.started = true;
                0
            };

            for i in 0..overlap {
                let w = i as f32 / overlap as f32;
                for c in 0..ch {
                    let a = self.tail[i * ch + c];
                    let b = self.input[(offset + i) * ch + c];
                    output.push(a * (1.0 - w) + b * w);
                }
            }
            let body = (offset + overlap) * ch;
            output.extend_from_slice(&self.input[body..body + middle * ch]);
            let tail_start = body + middle * ch;
            self.tail.clear();
            self.tail
                .extend_from_slice(&self.input[tail_start..tail_start + overlap * ch]);

            self.skip_fraction += nominal_skip;
            let skip = self.skip_fraction as usize;
            self.skip_fraction -= skip as f64;
            self.input.drain(..skip * ch);
        }
    }

    /// 归一化互相关最大的位置：先按步长粗搜，再在附近逐帧细搜。
    fn best_offset(&self) -> usize {
        let ch = self.channels;
        let mono = |buf: &[f32], frame: usize| buf[frame * ch..frame * ch + ch].iter().sum::<f32>();
        let reference: Vec<f32> = (0..self.overlap).map(|i| mono(&self.tail, i)).collect();
        let candidates: Vec<f32> = (0..self.seek + self.overlap).map(|i| mono(&self.input, i)).collect();

        let score = |offset: usize| {
            let window = &candidates[offset..offset + self.overlap];
            let (mut corr, mut energy) = (0.0_f32, 0.0_f32);
            for (r, x) in reference.iter().zip(window) {
                corr += r * x;
                energy += x * x;
            }
            corr / (energy + 1e-9).sqrt()
        };
        let best_in = |range: &mut dyn Iterator<Item = usize>| {
            range.fold((0, f32::MIN), |best, o| {
                let s = score(o);
                if s > best.1 { (o, s) } else { best }
            })
        };

        let (coarse, _) = best_in(&mut (0..self.seek).step_by(COARSE_STEP));
        let from = coarse.saturating_sub(COARSE_STEP - 1);
        let to = (coarse + COARSE_STEP).min(self.seek);
        best_in(&mut (from..to)).0
    }

    /// 线性插值重采样：读取速度为 `pitch` 倍。
    fn resample(&mut self, input: &[f32], output: &mut Vec<f32>) {
        let ch = self.channels;
        if self.pitch == 1.0 && self.resample_input.is_empty() {
            output.extend_from_slice(input);
            return;
        }
        self.resample_input.extend_from_slice(input);
        let frames = self.resample_input.len() / ch;
        while self.resample_pos + 1.0 < frames as f64 {
            let i = self.resample_pos as usize;
            let f = (self.resample_pos - i as f64) as f32;
            for c in 0..ch {
                let a = self.resample_input[i * ch + c];
                let b = self.resample_input[(i + 1) * ch + c];
                output.push(a + (b - a) * f);
            }
            self.resample_pos += self.pitch;
        }
        let consumed = (self.resample_pos as usize).min(frames);
        self.resample_input.drain(..consumed * ch);
        self.resample_pos -= consumed as f64;
    }
}

/// 离线渲染：用 [`Stretcher`] 处理整段音频。
pub fn render(pcm: &Pcm, rate: f32, pitch_semitones: i8) -> Pcm {
    let mut stretcher = Stretcher::new(pcm.sample_rate, pcm.channels);
    stretcher.set_rate(rate);
    stretcher.set_pitch(pitch_semitones);
    let mut samples = Vec::with_capacity((pcm.samples.len() as f64 / rate.max(0.25) as f64) as usize);
    stretcher.process(&pcm.samples, &mut samples);
    stretcher.flush(&mut samples);
    Pcm {
        sample_rate: pcm.sample_rate,
        channels: pcm.channels,
        samples,
    }
}

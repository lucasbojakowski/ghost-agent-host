use crate::audio::AudioBuffer;
use crate::model::{CompressorOperation, EqBandOperation, EqShape, MixOperation, MixPlan};

#[derive(Debug, Clone)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    fn peaking(sample_rate: f64, frequency: f64, gain_db: f64, q: f64) -> Self {
        let amplitude = 10.0_f64.powf(gain_db / 40.0);
        let omega = 2.0 * std::f64::consts::PI * frequency / sample_rate;
        let alpha = omega.sin() / (2.0 * q.max(0.05));
        let cos = omega.cos();
        let a0 = 1.0 + alpha / amplitude;
        Self {
            b0: (1.0 + alpha * amplitude) / a0,
            b1: (-2.0 * cos) / a0,
            b2: (1.0 - alpha * amplitude) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha / amplitude) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let input = input as f64;
        let output = self.b0 * input + self.z1;
        self.z1 = self.b1 * input - self.a1 * output + self.z2;
        self.z2 = self.b2 * input - self.a2 * output;
        output as f32
    }
}

pub fn render_mock_chain(source: &AudioBuffer, plan: &MixPlan) -> AudioBuffer {
    let mut output = source.clone();
    for operation in &plan.operations {
        match operation {
            MixOperation::EqBand { settings } if settings.enabled => {
                apply_eq(&mut output, settings);
            }
            MixOperation::Compressor { settings } if settings.enabled => {
                apply_compressor(&mut output, settings);
            }
            MixOperation::Bypass { .. } | MixOperation::EqBand { .. } | MixOperation::Compressor { .. } => {}
        }
    }
    output
}

fn apply_eq(audio: &mut AudioBuffer, settings: &EqBandOperation) {
    if !matches!(settings.shape, EqShape::Bell) {
        return;
    }
    for channel in &mut audio.channels {
        let mut filter = Biquad::peaking(
            audio.sample_rate as f64,
            settings.frequency_hz,
            settings.gain_db,
            settings.q,
        );
        for sample in channel {
            *sample = filter.process(*sample);
        }
    }
}

fn apply_compressor(audio: &mut AudioBuffer, settings: &CompressorOperation) {
    let sample_rate = audio.sample_rate as f64;
    let attack = (-1.0 / (settings.attack_ms.max(0.01) * 0.001 * sample_rate)).exp();
    let release = (-1.0 / (settings.release_ms.max(1.0) * 0.001 * sample_rate)).exp();
    let wet = (settings.mix_percent / 100.0).clamp(0.0, 1.0);
    let output_gain = 10.0_f64.powf(settings.output_gain_db / 20.0);
    let mut envelope = 0.0_f64;
    let mut gain = 1.0_f64;

    for frame in 0..audio.frames() {
        let detector = audio
            .channels
            .iter()
            .map(|channel| channel[frame].abs() as f64)
            .fold(0.0_f64, f64::max);
        let coefficient = if detector > envelope { attack } else { release };
        envelope = coefficient * envelope + (1.0 - coefficient) * detector;
        let level_db = 20.0 * envelope.max(1.0e-20).log10();
        let over = level_db - settings.threshold_db;
        let target_gain_db = if over <= -settings.knee_db * 0.5 {
            0.0
        } else if over >= settings.knee_db * 0.5 {
            -(over - over / settings.ratio.max(1.0)).min(settings.range_db.abs())
        } else {
            let x = over + settings.knee_db * 0.5;
            let compressed = x * x / (2.0 * settings.knee_db.max(0.01));
            -(compressed - compressed / settings.ratio.max(1.0)).min(settings.range_db.abs())
        };
        let target_gain = 10.0_f64.powf(target_gain_db / 20.0);
        gain = 0.95 * gain + 0.05 * target_gain;
        for channel in &mut audio.channels {
            let dry = channel[frame] as f64;
            let processed = dry * gain * output_gain;
            channel[frame] = (dry * (1.0 - wet) + processed * wet) as f32;
        }
    }
}

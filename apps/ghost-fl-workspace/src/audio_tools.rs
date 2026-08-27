use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use ghost_audio::{analyze_audio, read_audio, AnalysisConfig, AudioBuffer};
use ghost_tap::{
    discover_live_taps, find_live_tap, request_capture, wait_for_capture, TapCaptureCommand,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::project::{normalize_path, AssetRequest, WorkspaceProjectHub};

#[derive(Clone)]
pub(crate) struct AudioToolState {
    project: Arc<Mutex<WorkspaceProjectHub>>,
    analysis_root: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnalyzeAudioRequest {
    pub path: String,
    pub label: Option<String>,
    pub role: Option<String>,
    pub tempo_bpm: Option<f64>,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadAudioRequest {
    pub analysis_id: String,
    #[serde(default = "default_audio_view")]
    pub view: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompareAudioRequest {
    pub left_analysis_id: String,
    pub right_analysis_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TapArmRequest {
    pub instance_id: u32,
    pub duration_seconds: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TapCollectRequest {
    pub instance_id: u32,
    pub request_id: u64,
    #[serde(default = "default_tap_timeout")]
    pub timeout_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredAudioAnalysis {
    schema_version: String,
    analysis_id: String,
    source_path: String,
    label: String,
    role: String,
    acoustic: Value,
    musical: MusicalProjection,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct MusicalProjection {
    tempo_hint_bpm: Option<f64>,
    tempo_candidates: Vec<TempoCandidate>,
    timeline: Vec<TimelinePoint>,
    onsets: Vec<RhythmEvent>,
    pitch_events: Vec<PitchEvent>,
    section_candidates: Vec<SectionCandidate>,
    limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TempoCandidate {
    bpm: f64,
    confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimelinePoint {
    time_seconds: f64,
    rms_dbfs: f64,
    peak_dbfs: f64,
    transient_proxy: f64,
    zero_crossing_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RhythmEvent {
    seconds: f64,
    strength: f64,
    bar: Option<u32>,
    beat: Option<f64>,
    sixteenth: Option<u32>,
    offset_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PitchEvent {
    midi: i32,
    name: String,
    start_seconds: f64,
    duration_seconds: f64,
    start_beat: Option<f64>,
    duration_beats: Option<f64>,
    confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SectionCandidate {
    seconds: f64,
    bar: Option<u32>,
    change_score: f64,
}

impl AudioToolState {
    pub(crate) fn new(project: Arc<Mutex<WorkspaceProjectHub>>) -> Result<Self> {
        let analysis_root = project
            .lock()
            .map_err(|_| anyhow::anyhow!("workspace project lock poisoned"))?
            .analysis_root();
        fs::create_dir_all(&analysis_root).with_context(|| {
            format!(
                "failed to create workspace analysis cache {}",
                analysis_root.display()
            )
        })?;
        Ok(Self {
            project,
            analysis_root,
        })
    }

    pub(crate) fn analyze(&self, request: AnalyzeAudioRequest) -> Result<Value> {
        let path = normalize_path(&request.path)?;
        let role = request
            .role
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("other")
            .to_owned();
        let label = request
            .label
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .or_else(|| path.file_name().and_then(|value| value.to_str()).map(str::to_owned))
            .unwrap_or_else(|| "Audio asset".into());

        let existing_analysis_id = {
            let project = self
                .project
                .lock()
                .map_err(|_| anyhow::anyhow!("workspace project lock poisoned"))?;
            let current = project.current()?;
            current
                .assets
                .iter()
                .find(|asset| Path::new(&asset.path) == path)
                .and_then(|asset| asset.analysis_id.clone())
        };
        if !request.force {
            if let Some(analysis_id) = existing_analysis_id {
                if let Ok(stored) = self.load(&analysis_id) {
                    return Ok(compact_analysis_result(&stored, true));
                }
            }
        }

        let audio = read_audio(&path)
            .with_context(|| format!("failed to decode audio file {}", path.display()))?;
        let analysis = analyze_audio(
            path.to_string_lossy(),
            &audio,
            &AnalysisConfig::maximum(),
        )
        .context("maximum-quality Ghost audio analysis failed")?;
        let analysis_id = analysis.capture.content_hash.clone();
        let acoustic = serde_json::to_value(&analysis)?;
        let musical = analyze_musical_projection(&audio, request.tempo_bpm, &role);
        let stored = StoredAudioAnalysis {
            schema_version: "ghost.workspace-audio/1".into(),
            analysis_id: analysis_id.clone(),
            source_path: path.to_string_lossy().into_owned(),
            label: label.clone(),
            role: role.clone(),
            acoustic,
            musical,
        };
        self.save(&stored)?;

        let asset_id = {
            let project = self
                .project
                .lock()
                .map_err(|_| anyhow::anyhow!("workspace project lock poisoned"))?;
            let asset = project.ensure_asset(&path, Some(&label), Some(&role))?;
            project.set_asset_analysis(&asset.id, &analysis_id)?;
            asset.id
        };

        let mut result = compact_analysis_result(&stored, false);
        result["assetId"] = Value::String(asset_id);
        Ok(result)
    }

    pub(crate) fn read(&self, request: ReadAudioRequest) -> Result<Value> {
        let stored = self.load(&request.analysis_id)?;
        let view = request.view.trim().to_ascii_lowercase();
        let value = match view.as_str() {
            "summary" => compact_analysis_result(&stored, true),
            "acoustic" => json!({
                "analysisId": stored.analysis_id,
                "sourcePath": stored.source_path,
                "role": stored.role,
                "capture": stored.acoustic.get("capture"),
                "signal": stored.acoustic.get("signal")
            }),
            "timeline" => json!({
                "analysisId": stored.analysis_id,
                "timeline": stored.musical.timeline,
                "sectionCandidates": stored.musical.section_candidates
            }),
            "rhythm" | "dawprojection" | "daw_projection" => json!({
                "analysisId": stored.analysis_id,
                "tempoHintBpm": stored.musical.tempo_hint_bpm,
                "tempoCandidates": stored.musical.tempo_candidates,
                "onsets": stored.musical.onsets,
                "limitations": stored.musical.limitations
            }),
            "pitch" => json!({
                "analysisId": stored.analysis_id,
                "pitchEvents": stored.musical.pitch_events,
                "limitations": stored.musical.limitations
            }),
            "sections" => json!({
                "analysisId": stored.analysis_id,
                "sectionCandidates": stored.musical.section_candidates,
                "limitations": stored.musical.limitations
            }),
            other => bail!(
                "unknown audio analysis view `{other}`; use summary, acoustic, timeline, rhythm, pitch, sections, or dawProjection"
            ),
        };
        Ok(value)
    }

    pub(crate) fn compare(&self, request: CompareAudioRequest) -> Result<Value> {
        let left = self.load(&request.left_analysis_id)?;
        let right = self.load(&request.right_analysis_id)?;
        let fields = [
            ("integratedLufs", "/signal/loudness/integrated_lufs"),
            ("rmsDbfs", "/signal/loudness/rms_dbfs"),
            ("crestFactorDb", "/signal/loudness/crest_factor_db"),
            ("centroidHz", "/signal/spectrum/centroid_hz"),
            ("rolloff85Hz", "/signal/spectrum/rolloff_85_hz"),
            ("spectralFlatness", "/signal/spectrum/flatness"),
            ("transientDensityHz", "/signal/dynamics/transient_density_hz"),
            ("attackStrengthP90", "/signal/dynamics/attack_strength_p90"),
            ("stereoCorrelation", "/signal/stereo/broadband_correlation"),
            ("midSideRatioDb", "/signal/stereo/mid_side_ratio_db"),
        ];
        let mut deltas = serde_json::Map::new();
        for (name, pointer) in fields {
            let left_value = number_at(&left.acoustic, pointer);
            let right_value = number_at(&right.acoustic, pointer);
            deltas.insert(
                name.into(),
                json!({
                    "left": left_value,
                    "right": right_value,
                    "deltaRightMinusLeft": match (left_value, right_value) {
                        (Some(left), Some(right)) => Some(right - left),
                        _ => None
                    }
                }),
            );
        }

        let band_names = [
            "sub_db",
            "bass_db",
            "low_mid_db",
            "mid_db",
            "high_mid_db",
            "presence_db",
            "air_db",
        ];
        let mut bands = serde_json::Map::new();
        for band in band_names {
            let pointer = format!("/signal/spectrum/bands/{band}");
            let left_value = number_at(&left.acoustic, &pointer);
            let right_value = number_at(&right.acoustic, &pointer);
            bands.insert(
                band.trim_end_matches("_db").into(),
                json!({
                    "leftDb": left_value,
                    "rightDb": right_value,
                    "deltaDb": match (left_value, right_value) {
                        (Some(left), Some(right)) => Some(right - left),
                        _ => None
                    }
                }),
            );
        }

        Ok(json!({
            "left": {"analysisId": left.analysis_id, "label": left.label, "role": left.role},
            "right": {"analysisId": right.analysis_id, "label": right.label, "role": right.role},
            "deltas": deltas,
            "bandDeltas": bands
        }))
    }

    pub(crate) fn list_taps(&self) -> Result<Value> {
        Ok(serde_json::to_value(discover_live_taps()?)?)
    }

    pub(crate) fn arm_tap(&self, request: TapArmRequest) -> Result<Value> {
        let tap = find_live_tap(request.instance_id)?;
        let command = TapCaptureCommand::new(request.duration_seconds)?;
        request_capture(&tap, &command)?;
        Ok(json!({
            "instanceId": tap.instance_id,
            "processId": tap.process_id,
            "requestId": command.request_id,
            "durationSeconds": command.duration_seconds,
            "thresholdDbfs": command.threshold_dbfs,
            "preRollMs": command.pre_roll_ms,
            "nextStep": "Start FL Studio playback now, then call ghost_tap_collect with this requestId."
        }))
    }

    pub(crate) fn collect_tap(&self, request: TapCollectRequest) -> Result<Value> {
        if !request.timeout_seconds.is_finite()
            || !(0.1..=120.0).contains(&request.timeout_seconds)
        {
            bail!("timeoutSeconds must be between 0.1 and 120 seconds");
        }
        let tap = find_live_tap(request.instance_id)?;
        let artifact = wait_for_capture(
            &tap,
            request.request_id,
            Duration::from_secs_f64(request.timeout_seconds),
        )?;
        Ok(serde_json::to_value(artifact)?)
    }

    fn load(&self, analysis_id: &str) -> Result<StoredAudioAnalysis> {
        validate_analysis_id(analysis_id)?;
        let path = self.analysis_root.join(format!("{analysis_id}.json"));
        let bytes = fs::read(&path)
            .with_context(|| format!("unknown audio analysis `{analysis_id}`"))?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("invalid cached audio analysis {}", path.display()))
    }

    fn save(&self, analysis: &StoredAudioAnalysis) -> Result<()> {
        validate_analysis_id(&analysis.analysis_id)?;
        let path = self
            .analysis_root
            .join(format!("{}.json", analysis.analysis_id));
        fs::write(&path, serde_json::to_vec_pretty(analysis)?)
            .with_context(|| format!("failed to cache audio analysis {}", path.display()))?;
        Ok(())
    }
}

fn compact_analysis_result(analysis: &StoredAudioAnalysis, cached: bool) -> Value {
    let signal = analysis.acoustic.get("signal").unwrap_or(&Value::Null);
    let capture = analysis.acoustic.get("capture").unwrap_or(&Value::Null);
    json!({
        "analysisId": analysis.analysis_id,
        "cached": cached,
        "file": analysis.source_path,
        "label": analysis.label,
        "role": analysis.role,
        "durationSeconds": capture.get("duration_seconds"),
        "sampleRate": capture.get("sample_rate"),
        "channels": capture.get("channels"),
        "summary": {
            "integratedLufs": signal.pointer("/loudness/integrated_lufs"),
            "rmsDbfs": signal.pointer("/loudness/rms_dbfs"),
            "crestFactorDb": signal.pointer("/loudness/crest_factor_db"),
            "centroidHz": signal.pointer("/spectrum/centroid_hz"),
            "rolloff85Hz": signal.pointer("/spectrum/rolloff_85_hz"),
            "transientDensityHz": signal.pointer("/dynamics/transient_density_hz"),
            "stereoCorrelation": signal.pointer("/stereo/broadband_correlation"),
            "tempoCandidates": analysis.musical.tempo_candidates,
            "sectionCandidateCount": analysis.musical.section_candidates.len(),
            "pitchEventCount": analysis.musical.pitch_events.len()
        },
        "availableViews": ["summary", "acoustic", "timeline", "rhythm", "pitch", "sections", "dawProjection"]
    })
}

fn analyze_musical_projection(
    audio: &AudioBuffer,
    tempo_hint_bpm: Option<f64>,
    role: &str,
) -> MusicalProjection {
    let mono = audio.mono_mix();
    let timeline = temporal_map(&mono, audio.sample_rate);
    let raw_onsets = detect_onsets(&mono, audio.sample_rate);
    let tempo_candidates = tempo_candidates(&raw_onsets);
    let projected_tempo = tempo_hint_bpm.or_else(|| tempo_candidates.first().map(|item| item.bpm));
    let onsets = raw_onsets
        .into_iter()
        .map(|(seconds, strength)| rhythm_event(seconds, strength, projected_tempo))
        .collect();
    let role_lower = role.to_ascii_lowercase();
    let pitch_events = if role_lower.contains("bass")
        || role_lower.contains("lead")
        || role_lower.contains("melody")
        || role_lower.contains("monophonic")
    {
        pitch_projection(&mono, audio.sample_rate, projected_tempo)
    } else {
        Vec::new()
    };
    let section_candidates = section_candidates(&timeline, projected_tempo);
    let mut limitations = vec![
        "Tempo/onset/section values are lightweight deterministic musical projections, not ground truth.".into(),
        "Section boundaries are acoustic change candidates; semantic names belong to producer/model interpretation.".into(),
    ];
    if pitch_events.is_empty() {
        limitations.push(
            "Pitch transcription is only attempted for assets explicitly labelled as bass/lead/melody/monophonic.".into(),
        );
    } else {
        limitations.push(
            "Pitch events use monophonic autocorrelation and may contain octave errors, slides or unstable-note ambiguity.".into(),
        );
    }

    MusicalProjection {
        tempo_hint_bpm,
        tempo_candidates,
        timeline,
        onsets,
        pitch_events,
        section_candidates,
        limitations,
    }
}

fn temporal_map(samples: &[f32], sample_rate: u32) -> Vec<TimelinePoint> {
    let window = sample_rate as usize;
    if samples.is_empty() || window == 0 {
        return Vec::new();
    }
    samples
        .chunks(window)
        .enumerate()
        .filter(|(_, chunk)| chunk.len() >= window / 4)
        .map(|(index, chunk)| {
            let rms = rms(chunk);
            let peak = chunk
                .iter()
                .map(|sample| sample.abs() as f64)
                .fold(0.0_f64, f64::max);
            let transient = chunk
                .windows(2)
                .map(|pair| (pair[1] - pair[0]).abs() as f64)
                .sum::<f64>()
                / chunk.len().max(1) as f64;
            let crossings = chunk
                .windows(2)
                .filter(|pair| (pair[0] < 0.0) != (pair[1] < 0.0))
                .count();
            TimelinePoint {
                time_seconds: index as f64,
                rms_dbfs: gain_to_db(rms),
                peak_dbfs: gain_to_db(peak),
                transient_proxy: transient,
                zero_crossing_rate: crossings as f64 / chunk.len().max(1) as f64,
            }
        })
        .collect()
}

fn detect_onsets(samples: &[f32], sample_rate: u32) -> Vec<(f64, f64)> {
    let hop = ((sample_rate as f64 * 0.02).round() as usize).max(1);
    let window = ((sample_rate as f64 * 0.04).round() as usize).max(hop);
    if samples.len() < window {
        return Vec::new();
    }
    let mut energies = Vec::new();
    let mut start = 0;
    while start + window <= samples.len() {
        energies.push(rms(&samples[start..start + window]));
        start += hop;
    }
    if energies.len() < 3 {
        return Vec::new();
    }
    let novelty = energies
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).max(0.0))
        .collect::<Vec<_>>();
    let mut sorted = novelty.clone();
    sorted.sort_by(|left, right| left.total_cmp(right));
    let median = sorted[sorted.len() / 2];
    let mean = novelty.iter().sum::<f64>() / novelty.len().max(1) as f64;
    let threshold = (median * 3.0).max(mean * 1.6).max(1.0e-5);
    let mut events = Vec::new();
    let min_gap = (0.08 / 0.02) as usize;
    let mut last_index = usize::MAX;
    for index in 1..novelty.len().saturating_sub(1) {
        let value = novelty[index];
        if value < threshold || value < novelty[index - 1] || value < novelty[index + 1] {
            continue;
        }
        if last_index != usize::MAX && index.saturating_sub(last_index) < min_gap {
            continue;
        }
        let strength = (value / (threshold * 4.0)).clamp(0.0, 1.0);
        events.push((index as f64 * hop as f64 / sample_rate as f64, strength));
        last_index = index;
    }
    events
}

fn tempo_candidates(onsets: &[(f64, f64)]) -> Vec<TempoCandidate> {
    if onsets.len() < 3 {
        return Vec::new();
    }
    let mut histogram = BTreeMap::<i32, usize>::new();
    for pair in onsets.windows(2) {
        let delta = pair[1].0 - pair[0].0;
        if !(0.12..=2.0).contains(&delta) {
            continue;
        }
        let mut bpm = 60.0 / delta;
        while bpm < 60.0 {
            bpm *= 2.0;
        }
        while bpm > 190.0 {
            bpm /= 2.0;
        }
        *histogram.entry(bpm.round() as i32).or_default() += 1;
    }
    let total = histogram.values().sum::<usize>().max(1) as f64;
    let mut candidates = histogram
        .into_iter()
        .map(|(bpm, count)| TempoCandidate {
            bpm: bpm as f64,
            confidence: count as f64 / total,
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .confidence
            .total_cmp(&left.confidence)
            .then_with(|| left.bpm.total_cmp(&right.bpm))
    });
    candidates.truncate(3);
    candidates
}

fn rhythm_event(seconds: f64, strength: f64, bpm: Option<f64>) -> RhythmEvent {
    let Some(bpm) = bpm.filter(|value| value.is_finite() && *value > 0.0) else {
        return RhythmEvent {
            seconds,
            strength,
            bar: None,
            beat: None,
            sixteenth: None,
            offset_ms: None,
        };
    };
    let absolute_beat = seconds * bpm / 60.0;
    let nearest_sixteenth = (absolute_beat * 4.0).round();
    let quantized_beat = nearest_sixteenth / 4.0;
    let offset_ms = (absolute_beat - quantized_beat) * 60_000.0 / bpm;
    let bar_zero = (absolute_beat / 4.0).floor();
    let beat_in_bar = absolute_beat - bar_zero * 4.0 + 1.0;
    RhythmEvent {
        seconds,
        strength,
        bar: Some(bar_zero as u32 + 1),
        beat: Some(beat_in_bar),
        sixteenth: Some((nearest_sixteenth.rem_euclid(16.0) as u32) + 1),
        offset_ms: Some(offset_ms),
    }
}

fn pitch_projection(samples: &[f32], sample_rate: u32, bpm: Option<f64>) -> Vec<PitchEvent> {
    if samples.is_empty() || sample_rate == 0 {
        return Vec::new();
    }
    let downsample = (sample_rate / 8_000).max(1) as usize;
    let rate = sample_rate as f64 / downsample as f64;
    let reduced = samples.iter().step_by(downsample).copied().collect::<Vec<_>>();
    let window = (rate * 0.08).round() as usize;
    let hop = (rate * 0.25).round() as usize;
    if window < 32 || reduced.len() < window {
        return Vec::new();
    }
    let min_lag = (rate / 1_000.0).floor().max(2.0) as usize;
    let max_lag = (rate / 45.0).ceil() as usize;
    let mut frames = Vec::<(f64, i32, f64)>::new();
    let mut start = 0;
    while start + window <= reduced.len() {
        let frame = &reduced[start..start + window];
        if gain_to_db(rms(frame)) > -48.0 {
            if let Some((frequency, confidence)) = estimate_pitch(frame, rate, min_lag, max_lag) {
                if confidence >= 0.55 && frequency.is_finite() && (45.0..=1_000.0).contains(&frequency) {
                    let midi = (69.0 + 12.0 * (frequency / 440.0).log2()).round() as i32;
                    frames.push((start as f64 / rate, midi, confidence));
                }
            }
        }
        start += hop.max(1);
    }

    let mut events = Vec::<PitchEvent>::new();
    for (seconds, midi, confidence) in frames {
        if let Some(last) = events.last_mut() {
            let expected_end = last.start_seconds + last.duration_seconds;
            if last.midi == midi && seconds - expected_end <= 0.30 {
                last.duration_seconds = (seconds + hop as f64 / rate) - last.start_seconds;
                last.confidence = (last.confidence + confidence) / 2.0;
                last.duration_beats = bpm.map(|value| last.duration_seconds * value / 60.0);
                continue;
            }
        }
        events.push(PitchEvent {
            midi,
            name: midi_name(midi),
            start_seconds: seconds,
            duration_seconds: hop as f64 / rate,
            start_beat: bpm.map(|value| seconds * value / 60.0 + 1.0),
            duration_beats: bpm.map(|value| hop as f64 / rate * value / 60.0),
            confidence,
        });
        if events.len() >= 1_000 {
            break;
        }
    }
    events
}

fn estimate_pitch(
    samples: &[f32],
    sample_rate: f64,
    min_lag: usize,
    max_lag: usize,
) -> Option<(f64, f64)> {
    let mean = samples.iter().map(|value| *value as f64).sum::<f64>() / samples.len() as f64;
    let mut best_lag = 0;
    let mut best_score = 0.0_f64;
    for lag in min_lag..=max_lag.min(samples.len().saturating_sub(2)) {
        let mut dot = 0.0;
        let mut left_energy = 0.0;
        let mut right_energy = 0.0;
        for index in 0..samples.len() - lag {
            let left = samples[index] as f64 - mean;
            let right = samples[index + lag] as f64 - mean;
            dot += left * right;
            left_energy += left * left;
            right_energy += right * right;
        }
        let score = dot / (left_energy.sqrt() * right_energy.sqrt()).max(1.0e-12);
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }
    (best_lag > 0).then_some((sample_rate / best_lag as f64, best_score.clamp(0.0, 1.0)))
}

fn section_candidates(timeline: &[TimelinePoint], bpm: Option<f64>) -> Vec<SectionCandidate> {
    if timeline.len() < 3 {
        return Vec::new();
    }
    let mut scored = timeline
        .windows(2)
        .map(|pair| {
            let rms_delta = (pair[1].rms_dbfs - pair[0].rms_dbfs).abs() / 6.0;
            let transient_scale = pair[0].transient_proxy.abs().max(1.0e-6);
            let transient_delta = (pair[1].transient_proxy - pair[0].transient_proxy).abs()
                / transient_scale;
            let zcr_delta = (pair[1].zero_crossing_rate - pair[0].zero_crossing_rate).abs() * 8.0;
            (pair[1].time_seconds, rms_delta + transient_delta + zcr_delta)
        })
        .collect::<Vec<_>>();
    let mean = scored.iter().map(|(_, score)| *score).sum::<f64>() / scored.len() as f64;
    let variance = scored
        .iter()
        .map(|(_, score)| (*score - mean).powi(2))
        .sum::<f64>()
        / scored.len() as f64;
    let threshold = mean + variance.sqrt() * 1.25;
    scored.retain(|(_, score)| *score >= threshold);
    scored.sort_by(|left, right| right.1.total_cmp(&left.1));
    scored.truncate(24);
    scored.sort_by(|left, right| left.0.total_cmp(&right.0));

    let mut accepted = Vec::new();
    let mut last_seconds = -100.0;
    for (seconds, score) in scored {
        if seconds - last_seconds < 6.0 {
            continue;
        }
        accepted.push(SectionCandidate {
            seconds,
            bar: bpm.map(|value| (seconds * value / 60.0 / 4.0).floor() as u32 + 1),
            change_score: score,
        });
        last_seconds = seconds;
    }
    accepted
}

fn number_at(value: &Value, pointer: &str) -> Option<f64> {
    value.pointer(pointer).and_then(Value::as_f64)
}

fn rms(samples: &[f32]) -> f64 {
    (samples
        .iter()
        .map(|sample| (*sample as f64).powi(2))
        .sum::<f64>()
        / samples.len().max(1) as f64)
        .sqrt()
}

fn gain_to_db(value: f64) -> f64 {
    20.0 * value.max(1.0e-12).log10()
}

fn midi_name(midi: i32) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let pitch_class = midi.rem_euclid(12) as usize;
    let octave = midi.div_euclid(12) - 1;
    format!("{}{}", NAMES[pitch_class], octave)
}

fn validate_analysis_id(analysis_id: &str) -> Result<()> {
    if analysis_id.len() != 64 || !analysis_id.chars().all(|character| character.is_ascii_hexdigit()) {
        bail!("invalid audio analysis id");
    }
    Ok(())
}

fn default_audio_view() -> String {
    "summary".into()
}

fn default_tap_timeout() -> f64 {
    30.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rhythm_projection_maps_to_bar_grid() {
        let event = rhythm_event(1.0, 0.8, Some(120.0));
        assert_eq!(event.bar, Some(1));
        assert_eq!(event.beat, Some(3.0));
        assert!(event.offset_ms.unwrap().abs() < 0.001);
    }

    #[test]
    fn midi_names_are_stable() {
        assert_eq!(midi_name(38), "D2");
        assert_eq!(midi_name(60), "C4");
    }

    #[test]
    fn analysis_ids_reject_paths() {
        assert!(validate_analysis_id("../analysis").is_err());
        assert!(validate_analysis_id(&"a".repeat(64)).is_ok());
    }
}

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureState {
    Idle,
    Armed,
    Capturing,
    Complete,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CaptureError {
    #[error("capture target exceeds preallocated capacity")]
    CapacityExceeded,
    #[error("tap configuration must contain unique, non-empty names")]
    InvalidTapConfiguration,
    #[error("tap count or name does not match the capture graph")]
    TapMismatch,
    #[error("capture channel count does not match engine configuration")]
    ChannelMismatch,
    #[error("capture taps have different frame counts")]
    FrameMismatch,
    #[error("capture is not complete")]
    NotComplete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureTap {
    pub name: String,
    pub channels: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaptureSnapshot {
    pub sample_rate: u32,
    pub channels: usize,
    pub frames: usize,
    pub taps: Vec<CaptureTap>,
}

/// Borrowed block for one configured tap. Callers should build this view outside the audio callback
/// and reuse it. `push_block` never allocates while armed capacity is respected.
#[derive(Debug, Clone, Copy)]
pub struct TapBlock<'a> {
    pub name: &'a str,
    pub channels: &'a [&'a [f32]],
}

struct TapBuffer {
    name: String,
    channels: Vec<Vec<f32>>,
}

pub struct CaptureGraph {
    sample_rate: u32,
    channels: usize,
    maximum_frames: usize,
    target_frames: usize,
    captured_frames: usize,
    state: CaptureState,
    taps: Vec<TapBuffer>,
}

impl CaptureGraph {
    pub fn new(
        sample_rate: u32,
        channels: usize,
        maximum_frames: usize,
        tap_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, CaptureError> {
        let mut names: Vec<String> = tap_names.into_iter().map(Into::into).collect();
        names.sort();
        if names.is_empty()
            || names.iter().any(String::is_empty)
            || names.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(CaptureError::InvalidTapConfiguration);
        }
        let taps = names
            .into_iter()
            .map(|name| TapBuffer {
                name,
                channels: (0..channels)
                    .map(|_| Vec::with_capacity(maximum_frames))
                    .collect(),
            })
            .collect();
        Ok(Self {
            sample_rate,
            channels,
            maximum_frames,
            target_frames: 0,
            captured_frames: 0,
            state: CaptureState::Idle,
            taps,
        })
    }

    pub fn state(&self) -> CaptureState {
        self.state
    }

    pub fn tap_names(&self) -> impl Iterator<Item = &str> {
        self.taps.iter().map(|tap| tap.name.as_str())
    }

    pub fn captured_frames(&self) -> usize {
        self.captured_frames
    }

    pub fn arm(&mut self, target_frames: usize) -> Result<(), CaptureError> {
        if target_frames > self.maximum_frames {
            return Err(CaptureError::CapacityExceeded);
        }
        self.target_frames = target_frames;
        self.captured_frames = 0;
        self.clear_taps();
        self.state = CaptureState::Armed;
        Ok(())
    }

    pub fn start(&mut self) {
        if self.state == CaptureState::Armed {
            self.state = CaptureState::Capturing;
        }
    }

    pub fn push_block(&mut self, blocks: &[TapBlock<'_>]) -> Result<CaptureState, CaptureError> {
        if self.state != CaptureState::Capturing {
            return Ok(self.state);
        }
        if blocks.len() != self.taps.len()
            || blocks
                .iter()
                .zip(&self.taps)
                .any(|(block, tap)| block.name != tap.name)
        {
            return Err(CaptureError::TapMismatch);
        }
        if blocks
            .iter()
            .any(|block| block.channels.len() != self.channels)
        {
            return Err(CaptureError::ChannelMismatch);
        }
        let block_frames = blocks
            .first()
            .and_then(|tap| tap.channels.first())
            .map_or(0, |channel| channel.len());
        if blocks
            .iter()
            .flat_map(|tap| tap.channels)
            .any(|channel| channel.len() != block_frames)
        {
            return Err(CaptureError::FrameMismatch);
        }
        let accepted = self
            .target_frames
            .saturating_sub(self.captured_frames)
            .min(block_frames);
        for (source, target) in blocks.iter().zip(&mut self.taps) {
            for (source_channel, target_channel) in source.channels.iter().zip(&mut target.channels)
            {
                target_channel.extend_from_slice(&source_channel[..accepted]);
            }
        }
        self.captured_frames += accepted;
        if self.captured_frames >= self.target_frames {
            self.state = CaptureState::Complete;
        }
        Ok(self.state)
    }

    pub fn snapshot(&self) -> Result<CaptureSnapshot, CaptureError> {
        if self.state != CaptureState::Complete {
            return Err(CaptureError::NotComplete);
        }
        Ok(CaptureSnapshot {
            sample_rate: self.sample_rate,
            channels: self.channels,
            frames: self.captured_frames,
            taps: self
                .taps
                .iter()
                .map(|tap| CaptureTap {
                    name: tap.name.clone(),
                    channels: tap.channels.clone(),
                })
                .collect(),
        })
    }

    pub fn reset(&mut self) {
        self.target_frames = 0;
        self.captured_frames = 0;
        self.state = CaptureState::Idle;
        self.clear_taps();
    }

    fn clear_taps(&mut self) {
        for tap in &mut self.taps {
            for channel in &mut tap.channels {
                channel.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_a_caller_defined_graph() {
        let mut graph =
            CaptureGraph::new(48_000, 2, 16, ["input", "post_comp", "post_eq"]).unwrap();
        graph.arm(3).unwrap();
        graph.start();
        let left = [0.1, 0.2, 0.3, 0.4];
        let right = [-0.1, -0.2, -0.3, -0.4];
        let channels: &[&[f32]] = &[&left, &right];
        let blocks = [
            TapBlock {
                name: "input",
                channels,
            },
            TapBlock {
                name: "post_comp",
                channels,
            },
            TapBlock {
                name: "post_eq",
                channels,
            },
        ];
        assert_eq!(graph.push_block(&blocks).unwrap(), CaptureState::Complete);
        let snapshot = graph.snapshot().unwrap();
        assert_eq!(snapshot.frames, 3);
        assert_eq!(snapshot.taps[1].name, "post_comp");
        assert_eq!(snapshot.taps[2].channels[0], vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn rejects_out_of_order_taps_without_mutation() {
        let mut graph = CaptureGraph::new(48_000, 1, 4, ["a", "b"]).unwrap();
        graph.arm(2).unwrap();
        graph.start();
        let samples = [0.0, 1.0];
        let channels: &[&[f32]] = &[&samples];
        let blocks = [
            TapBlock {
                name: "b",
                channels,
            },
            TapBlock {
                name: "a",
                channels,
            },
        ];
        assert_eq!(graph.push_block(&blocks), Err(CaptureError::TapMismatch));
        assert_eq!(graph.captured_frames(), 0);
    }
}

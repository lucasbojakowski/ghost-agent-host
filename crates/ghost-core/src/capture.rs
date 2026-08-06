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
    #[error("capture channel count does not match engine configuration")]
    ChannelMismatch,
    #[error("capture taps have different frame counts")]
    FrameMismatch,
    #[error("capture is not complete")]
    NotComplete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureTriplet {
    pub sample_rate: u32,
    pub channels: usize,
    pub frames: usize,
    pub input: Vec<Vec<f32>>,
    pub post_eq: Vec<Vec<f32>>,
    pub output: Vec<Vec<f32>>,
}

pub struct CaptureEngine {
    sample_rate: u32,
    channels: usize,
    maximum_frames: usize,
    target_frames: usize,
    captured_frames: usize,
    state: CaptureState,
    input: Vec<Vec<f32>>,
    post_eq: Vec<Vec<f32>>,
    output: Vec<Vec<f32>>,
}

impl CaptureEngine {
    pub fn new(sample_rate: u32, channels: usize, maximum_frames: usize) -> Self {
        let allocate = || {
            (0..channels)
                .map(|_| Vec::with_capacity(maximum_frames))
                .collect::<Vec<_>>()
        };
        Self {
            sample_rate,
            channels,
            maximum_frames,
            target_frames: 0,
            captured_frames: 0,
            state: CaptureState::Idle,
            input: allocate(),
            post_eq: allocate(),
            output: allocate(),
        }
    }

    pub fn state(&self) -> CaptureState {
        self.state
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
        for tap in [&mut self.input, &mut self.post_eq, &mut self.output] {
            for channel in tap.iter_mut() {
                channel.clear();
            }
        }
        self.state = CaptureState::Armed;
        Ok(())
    }

    pub fn start(&mut self) {
        if self.state == CaptureState::Armed {
            self.state = CaptureState::Capturing;
        }
    }

    pub fn push_block(
        &mut self,
        input: &[&[f32]],
        post_eq: &[&[f32]],
        output: &[&[f32]],
    ) -> Result<CaptureState, CaptureError> {
        if self.state != CaptureState::Capturing {
            return Ok(self.state);
        }
        if input.len() != self.channels
            || post_eq.len() != self.channels
            || output.len() != self.channels
        {
            return Err(CaptureError::ChannelMismatch);
        }
        let block_frames = input.first().map_or(0, |channel| channel.len());
        let all_match = input
            .iter()
            .chain(post_eq)
            .chain(output)
            .all(|channel| channel.len() == block_frames);
        if !all_match {
            return Err(CaptureError::FrameMismatch);
        }

        let remaining = self.target_frames.saturating_sub(self.captured_frames);
        let accepted = remaining.min(block_frames);
        for channel in 0..self.channels {
            self.input[channel].extend_from_slice(&input[channel][..accepted]);
            self.post_eq[channel].extend_from_slice(&post_eq[channel][..accepted]);
            self.output[channel].extend_from_slice(&output[channel][..accepted]);
        }
        self.captured_frames += accepted;
        if self.captured_frames >= self.target_frames {
            self.state = CaptureState::Complete;
        }
        Ok(self.state)
    }

    pub fn snapshot(&self) -> Result<CaptureTriplet, CaptureError> {
        if self.state != CaptureState::Complete {
            return Err(CaptureError::NotComplete);
        }
        Ok(CaptureTriplet {
            sample_rate: self.sample_rate,
            channels: self.channels,
            frames: self.captured_frames,
            input: self.input.clone(),
            post_eq: self.post_eq.clone(),
            output: self.output.clone(),
        })
    }

    pub fn reset(&mut self) {
        self.target_frames = 0;
        self.captured_frames = 0;
        self.state = CaptureState::Idle;
        for tap in [&mut self.input, &mut self.post_eq, &mut self.output] {
            for channel in tap.iter_mut() {
                channel.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_exact_target_without_reallocating_capacity() {
        let mut capture = CaptureEngine::new(48_000, 2, 16);
        let capacities: Vec<usize> = capture.input.iter().map(Vec::capacity).collect();
        capture.arm(6).unwrap();
        capture.start();
        let left = [0.1, 0.2, 0.3, 0.4];
        let right = [-0.1, -0.2, -0.3, -0.4];
        assert_eq!(
            capture
                .push_block(&[&left, &right], &[&left, &right], &[&left, &right])
                .unwrap(),
            CaptureState::Capturing
        );
        assert_eq!(
            capture
                .push_block(&[&left, &right], &[&left, &right], &[&left, &right])
                .unwrap(),
            CaptureState::Complete
        );
        let snapshot = capture.snapshot().unwrap();
        assert_eq!(snapshot.frames, 6);
        assert_eq!(snapshot.input[0], vec![0.1, 0.2, 0.3, 0.4, 0.1, 0.2]);
        assert_eq!(capacities, capture.input.iter().map(Vec::capacity).collect::<Vec<_>>());
    }
}

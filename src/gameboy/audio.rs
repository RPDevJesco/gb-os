//! Audio output abstraction and software resampling
//!
//! This module provides:
//! - [`AudioOutput`] trait for platform-specific audio output
//! - [`AudioResampler`] for converting Game Boy audio to target sample rates
//! - [`BlipBuf`] - a simple band-limited synthesis buffer (replaces external blip_buf)

use alloc::vec;
use alloc::vec::Vec;

/// Audio output trait - implement this for your platform
pub trait AudioOutput: Send {
    /// Queue audio samples for playback
    /// Both buffers have the same length, representing stereo samples
    fn play(&mut self, left: &[f32], right: &[f32]);

    /// Get the output sample rate
    fn sample_rate(&self) -> u32;

    /// Check if the audio buffer has underflowed (ran out of samples)
    /// Used to sync audio after speed changes
    fn underflowed(&self) -> bool;
}

/// Null audio output that discards all samples
pub struct NullAudio {
    sample_rate: u32,
}

impl NullAudio {
    pub fn new(sample_rate: u32) -> Self {
        Self { sample_rate }
    }
}

impl AudioOutput for NullAudio {
    fn play(&mut self, _left: &[f32], _right: &[f32]) {}

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn underflowed(&self) -> bool {
        false
    }
}

/// Band-limited sample buffer for high-quality audio resampling
/// This is a simplified implementation inspired by blip_buf
pub struct BlipBuf {
    /// Sample accumulator buffer
    buffer: Vec<i32>,
    /// Current write position in the buffer
    offset: usize,
    /// Source clock rate
    clock_rate: f64,
    /// Target sample rate
    sample_rate: f64,
    /// Conversion factor
    factor: f64,
    /// Accumulated time (reserved for future use)
    _time_acc: f64,
    /// Last delta value for integration
    integrator: i32,
}

impl BlipBuf {
    /// Size of each sample in the buffer (reserved for future use)
    #[allow(dead_code)]
    const SAMPLE_SHIFT: usize = 16;
    
    /// Pre-calculated band-limited impulse (simplified sinc)
    /// This uses a 4-tap filter for efficiency while maintaining quality
    const IMPULSE: [i32; 8] = [
        0x0000, 0x0D15, 0x3587, 0x6000, 
        0x6000, 0x3587, 0x0D15, 0x0000,
    ];

    /// Create a new BlipBuf with the specified maximum sample count
    pub fn new(max_samples: u32) -> Self {
        let buf_size = (max_samples as usize + 8) * 2;
        Self {
            buffer: vec![0; buf_size],
            offset: 0,
            clock_rate: 1.0,
            sample_rate: 1.0,
            factor: 1.0,
            _time_acc: 0.0,
            integrator: 0,
        }
    }

    /// Set the clock and sample rates for resampling
    pub fn set_rates(&mut self, clock_rate: f64, sample_rate: f64) {
        self.clock_rate = clock_rate;
        self.sample_rate = sample_rate;
        self.factor = sample_rate / clock_rate;
    }

    /// Add a delta (change in amplitude) at the specified clock time
    #[inline]
    pub fn add_delta(&mut self, time: u32, delta: i32) {
        if delta == 0 {
            return;
        }

        // Convert clock time to sample position
        let sample_pos = (time as f64 * self.factor) as usize;
        let scaled = time as f64 * self.factor;
        let frac = ((scaled - (scaled as u32 as f64)) * 256.0) as usize;

        let buf_pos = self.offset + sample_pos;
        if buf_pos + 8 >= self.buffer.len() {
            return; // Prevent overflow
        }

        // Apply band-limited impulse
        let impulse_offset = frac >> 5; // 0-7 based on fractional position
        
        for i in 0..4 {
            let imp = Self::IMPULSE[(impulse_offset + i) & 7] as i32;
            self.buffer[buf_pos + i] += (delta * imp) >> 14;
        }
    }

    /// Mark the end of a frame at the specified clock time
    pub fn end_frame(&mut self, clock_duration: u32) {
        // Calculate how many samples this frame produced
        let sample_count = (clock_duration as f64 * self.factor) as usize;
        self.offset += sample_count;
    }

    /// Get the number of samples available for reading
    #[inline]
    pub fn samples_avail(&self) -> i32 {
        self.offset as i32
    }

    /// Read samples from the buffer
    /// Returns the number of samples actually read
    pub fn read_samples(&mut self, output: &mut [i16], _stereo: bool) -> usize {
        let count = core::cmp::min(output.len(), self.offset);
        
        for i in 0..count {
            // Integrate the deltas
            self.integrator += self.buffer[i];
            
            // Clamp to i16 range
            let sample = self.integrator.clamp(-32768, 32767) as i16;
            output[i] = sample;
            
            // Clear the buffer position
            self.buffer[i] = 0;
        }

        // Shift remaining data to the beginning
        if count < self.offset {
            self.buffer.copy_within(count..self.offset, 0);
        }
        self.offset -= count;

        count
    }

    /// Clear all buffered samples
    pub fn clear(&mut self) {
        for v in &mut self.buffer {
            *v = 0;
        }
        self.offset = 0;
        self.integrator = 0;
    }
}

/// Audio resampler for converting between sample rates
pub struct AudioResampler {
    source_rate: u32,
    target_rate: u32,
    buffer_left: Vec<f32>,
    buffer_right: Vec<f32>,
    accumulator: f64,
    last_left: f32,
    last_right: f32,
}

impl AudioResampler {
    /// Create a new resampler
    pub fn new(source_rate: u32, target_rate: u32) -> Self {
        Self {
            source_rate,
            target_rate,
            buffer_left: Vec::with_capacity(4096),
            buffer_right: Vec::with_capacity(4096),
            accumulator: 0.0,
            last_left: 0.0,
            last_right: 0.0,
        }
    }

    /// Process input samples and return resampled output
    pub fn process(
        &mut self,
        input_left: &[f32],
        input_right: &[f32],
    ) -> (&[f32], &[f32]) {
        self.buffer_left.clear();
        self.buffer_right.clear();

        if input_left.is_empty() {
            return (&self.buffer_left, &self.buffer_right);
        }

        let ratio = self.source_rate as f64 / self.target_rate as f64;
        let mut idx = 0usize;

        while idx < input_left.len() {
            let frac = (self.accumulator - (self.accumulator as u32 as f64)) as f32;

            // Linear interpolation
            let left = if idx > 0 {
                self.last_left * (1.0 - frac) + input_left[idx] * frac
            } else {
                input_left[idx]
            };

            let right = if idx > 0 {
                self.last_right * (1.0 - frac) + input_right[idx] * frac
            } else {
                input_right[idx]
            };

            self.buffer_left.push(left);
            self.buffer_right.push(right);

            self.accumulator += ratio;
            while self.accumulator >= 1.0 {
                if idx < input_left.len() {
                    self.last_left = input_left[idx];
                    self.last_right = input_right[idx];
                }
                idx += 1;
                self.accumulator -= 1.0;
            }
        }

        (&self.buffer_left, &self.buffer_right)
    }

    /// Reset the resampler state
    pub fn reset(&mut self) {
        self.buffer_left.clear();
        self.buffer_right.clear();
        self.accumulator = 0.0;
        self.last_left = 0.0;
        self.last_right = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blip_buf_basic() {
        let mut blip = BlipBuf::new(1024);
        blip.set_rates(4194304.0, 44100.0);

        blip.add_delta(0, 100);
        blip.add_delta(100, -100);
        blip.end_frame(1000);

        assert!(blip.samples_avail() > 0);
    }

    #[test]
    fn test_resampler() {
        let mut resampler = AudioResampler::new(44100, 48000);

        let input = [0.5f32; 100];
        let (left, right) = resampler.process(&input, &input);

        assert!(!left.is_empty());
        assert_eq!(left.len(), right.len());
    }
}

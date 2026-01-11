//! Game Boy audio emulation
//!
//! Emulates all 4 sound channels:
//! - Channel 1: Square wave with sweep
//! - Channel 2: Square wave
//! - Channel 3: Wave channel
//! - Channel 4: Noise channel

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::audio::{AudioOutput, BlipBuf};

const WAVE_PATTERN: [[i32; 8]; 4] = [
    [-1, -1, -1, -1, 1, -1, -1, -1],
    [-1, -1, -1, -1, 1, 1, -1, -1],
    [-1, -1, 1, 1, 1, 1, -1, -1],
    [1, 1, 1, 1, -1, -1, 1, 1],
];

const CLOCKS_PER_SECOND: u32 = 1 << 22;
const CLOCKS_PER_FRAME: u32 = CLOCKS_PER_SECOND / 512;
const OUTPUT_SAMPLE_COUNT: usize = 2000;
const SWEEP_DELAY_ZERO_PERIOD: u8 = 8;
const WAVE_INITIAL_DELAY: u32 = 4;

struct VolumeEnvelope {
    period: u8,
    goes_up: bool,
    delay: u8,
    initial_volume: u8,
    volume: u8,
}

impl VolumeEnvelope {
    fn new() -> Self {
        Self {
            period: 0,
            goes_up: false,
            delay: 0,
            initial_volume: 0,
            volume: 0,
        }
    }

    fn rb(&self, a: u16) -> u8 {
        match a {
            0xFF12 | 0xFF17 | 0xFF21 => {
                ((self.initial_volume & 0xF) << 4)
                    | (if self.goes_up { 0x08 } else { 0 })
                    | (self.period & 0x7)
            }
            _ => 0xFF,
        }
    }

    fn wb(&mut self, a: u16, v: u8) {
        match a {
            0xFF12 | 0xFF17 | 0xFF21 => {
                self.period = v & 0x7;
                self.goes_up = v & 0x8 != 0;
                self.initial_volume = v >> 4;
                self.volume = self.initial_volume;
            }
            0xFF14 | 0xFF19 | 0xFF23 if v & 0x80 != 0 => {
                self.delay = self.period;
                self.volume = self.initial_volume;
            }
            _ => {}
        }
    }

    fn step(&mut self) {
        if self.delay > 1 {
            self.delay -= 1;
        } else if self.delay == 1 {
            self.delay = self.period;
            if self.goes_up && self.volume < 15 {
                self.volume += 1;
            } else if !self.goes_up && self.volume > 0 {
                self.volume -= 1;
            }
        }
    }
}

struct LengthCounter {
    enabled: bool,
    value: u16,
    max: u16,
}

impl LengthCounter {
    fn new(max: u16) -> Self {
        Self {
            enabled: false,
            value: 0,
            max,
        }
    }

    fn is_active(&self) -> bool {
        self.value > 0
    }

    fn extra_step(frame_step: u8) -> bool {
        frame_step % 2 == 1
    }

    fn enable(&mut self, enable: bool, frame_step: u8) {
        let was_enabled = self.enabled;
        self.enabled = enable;
        if !was_enabled && Self::extra_step(frame_step) {
            self.step();
        }
    }

    fn set(&mut self, minus_value: u8) {
        self.value = self.max - minus_value as u16;
    }

    fn trigger(&mut self, frame_step: u8) {
        if self.value == 0 {
            self.value = self.max;
            if Self::extra_step(frame_step) {
                self.step();
            }
        }
    }

    fn step(&mut self) {
        if self.enabled && self.value > 0 {
            self.value -= 1;
        }
    }
}

struct SquareChannel {
    active: bool,
    dac_enabled: bool,
    duty: u8,
    phase: u8,
    length: LengthCounter,
    frequency: u16,
    period: u32,
    last_amp: i32,
    delay: u32,
    has_sweep: bool,
    sweep_enabled: bool,
    sweep_frequency: u16,
    sweep_delay: u8,
    sweep_period: u8,
    sweep_shift: u8,
    sweep_negate: bool,
    sweep_did_negate: bool,
    volume_envelope: VolumeEnvelope,
    blip: BlipBuf,
}

impl SquareChannel {
    fn new(sample_rate: u32, with_sweep: bool) -> Self {
        let mut blip = BlipBuf::new((OUTPUT_SAMPLE_COUNT + 1) as u32);
        blip.set_rates(CLOCKS_PER_SECOND as f64, sample_rate as f64);

        Self {
            active: false,
            dac_enabled: false,
            duty: 1,
            phase: 1,
            length: LengthCounter::new(64),
            frequency: 0,
            period: 2048,
            last_amp: 0,
            delay: 0,
            has_sweep: with_sweep,
            sweep_enabled: false,
            sweep_frequency: 0,
            sweep_delay: 0,
            sweep_period: 0,
            sweep_shift: 0,
            sweep_negate: false,
            sweep_did_negate: false,
            volume_envelope: VolumeEnvelope::new(),
            blip,
        }
    }

    fn on(&self) -> bool {
        self.active
    }

    fn rb(&self, a: u16) -> u8 {
        match a {
            0xFF10 => {
                0x80 | ((self.sweep_period & 0x7) << 4)
                    | (if self.sweep_negate { 0x8 } else { 0 })
                    | (self.sweep_shift & 0x7)
            }
            0xFF11 | 0xFF16 => ((self.duty & 3) << 6) | 0x3F,
            0xFF12 | 0xFF17 => self.volume_envelope.rb(a),
            0xFF13 | 0xFF18 => 0xFF,
            0xFF14 | 0xFF19 => 0x80 | (if self.length.enabled { 0x40 } else { 0 }) | 0x3F,
            _ => 0xFF,
        }
    }

    fn wb(&mut self, a: u16, v: u8, frame_step: u8) {
        match a {
            0xFF10 => {
                self.sweep_period = (v >> 4) & 0x7;
                self.sweep_shift = v & 0x7;
                let old_negate = self.sweep_negate;
                self.sweep_negate = v & 0x8 != 0;
                if old_negate && !self.sweep_negate && self.sweep_did_negate {
                    self.active = false;
                }
                self.sweep_did_negate = false;
            }
            0xFF11 | 0xFF16 => {
                self.duty = v >> 6;
                self.length.set(v & 0x3F);
            }
            0xFF12 | 0xFF17 => {
                self.dac_enabled = v & 0xF8 != 0;
                self.active = self.active && self.dac_enabled;
            }
            0xFF13 | 0xFF18 => {
                self.frequency = (self.frequency & 0x0700) | (v as u16);
                self.calculate_period();
            }
            0xFF14 | 0xFF19 => {
                self.frequency = (self.frequency & 0x00FF) | ((v as u16 & 0x07) << 8);
                self.calculate_period();

                self.length.enable(v & 0x40 != 0, frame_step);
                self.active &= self.length.is_active();

                if v & 0x80 != 0 {
                    if self.dac_enabled {
                        self.active = true;
                    }
                    self.length.trigger(frame_step);

                    if self.has_sweep {
                        self.sweep_frequency = self.frequency;
                        self.sweep_delay = if self.sweep_period != 0 {
                            self.sweep_period
                        } else {
                            SWEEP_DELAY_ZERO_PERIOD
                        };
                        self.sweep_enabled = self.sweep_period > 0 || self.sweep_shift > 0;
                        if self.sweep_shift > 0 {
                            self.sweep_calculate_frequency();
                        }
                    }
                }
            }
            _ => {}
        }
        self.volume_envelope.wb(a, v);
    }

    fn calculate_period(&mut self) {
        self.period = if self.frequency > 2047 {
            0
        } else {
            (2048 - self.frequency as u32) * 4
        };
    }

    fn run(&mut self, start_time: u32, end_time: u32) {
        if !self.active || self.period == 0 {
            if self.last_amp != 0 {
                self.blip.add_delta(start_time, -self.last_amp);
                self.last_amp = 0;
                self.delay = 0;
            }
            return;
        }

        let mut time = start_time + self.delay;
        let pattern = WAVE_PATTERN[self.duty as usize];
        let vol = self.volume_envelope.volume as i32;

        while time < end_time {
            let amp = vol * pattern[self.phase as usize];
            if amp != self.last_amp {
                self.blip.add_delta(time, amp - self.last_amp);
                self.last_amp = amp;
            }
            time += self.period;
            self.phase = (self.phase + 1) % 8;
        }

        self.delay = time - end_time;
    }

    fn step_length(&mut self) {
        self.length.step();
        self.active &= self.length.is_active();
    }

    fn sweep_calculate_frequency(&mut self) -> u16 {
        let offset = self.sweep_frequency >> self.sweep_shift;
        let newfreq = if self.sweep_negate {
            self.sweep_did_negate = true;
            self.sweep_frequency.wrapping_sub(offset)
        } else {
            self.sweep_frequency.wrapping_add(offset)
        };

        if newfreq > 2047 {
            self.active = false;
        }
        newfreq
    }

    fn step_sweep(&mut self) {
        if self.sweep_delay > 1 {
            self.sweep_delay -= 1;
        } else {
            self.sweep_delay = if self.sweep_period == 0 {
                SWEEP_DELAY_ZERO_PERIOD
            } else {
                self.sweep_period
            };

            if self.sweep_enabled && self.sweep_period != 0 {
                let newfreq = self.sweep_calculate_frequency();
                if newfreq <= 2047 && self.sweep_shift != 0 {
                    self.sweep_frequency = newfreq;
                    self.frequency = newfreq;
                    self.calculate_period();
                    self.sweep_calculate_frequency();
                }
            }
        }
    }
}

struct WaveChannel {
    active: bool,
    dac_enabled: bool,
    length: LengthCounter,
    frequency: u16,
    period: u32,
    last_amp: i32,
    delay: u32,
    volume_shift: u8,
    waveram: [u8; 16],
    current_wave: u8,
    dmg_mode: bool,
    sample_recently_accessed: bool,
    blip: BlipBuf,
}

impl WaveChannel {
    fn new(sample_rate: u32, dmg_mode: bool) -> Self {
        let mut blip = BlipBuf::new((OUTPUT_SAMPLE_COUNT + 1) as u32);
        blip.set_rates(CLOCKS_PER_SECOND as f64, sample_rate as f64);

        Self {
            active: false,
            dac_enabled: false,
            length: LengthCounter::new(256),
            frequency: 0,
            period: 2048,
            last_amp: 0,
            delay: 0,
            volume_shift: 0,
            waveram: [0; 16],
            current_wave: 0,
            dmg_mode,
            sample_recently_accessed: false,
            blip,
        }
    }

    fn rb(&self, a: u16) -> u8 {
        match a {
            0xFF1A => (if self.dac_enabled { 0x80 } else { 0 }) | 0x7F,
            0xFF1B => 0xFF,
            0xFF1C => 0x80 | ((self.volume_shift & 0b11) << 5) | 0x1F,
            0xFF1D => 0xFF,
            0xFF1E => 0x80 | (if self.length.enabled { 0x40 } else { 0 }) | 0x3F,
            0xFF30..=0xFF3F => {
                if !self.active {
                    self.waveram[a as usize - 0xFF30]
                } else if !self.dmg_mode || self.sample_recently_accessed {
                    self.waveram[self.current_wave as usize >> 1]
                } else {
                    0xFF
                }
            }
            _ => 0xFF,
        }
    }

    fn wb(&mut self, a: u16, v: u8, frame_step: u8) {
        match a {
            0xFF1A => {
                self.dac_enabled = v & 0x80 != 0;
                self.active = self.active && self.dac_enabled;
            }
            0xFF1B => self.length.set(v),
            0xFF1C => self.volume_shift = (v >> 5) & 0b11,
            0xFF1D => {
                self.frequency = (self.frequency & 0x0700) | (v as u16);
                self.calculate_period();
            }
            0xFF1E => {
                self.frequency = (self.frequency & 0x00FF) | ((v as u16 & 0b111) << 8);
                self.calculate_period();

                self.length.enable(v & 0x40 != 0, frame_step);
                self.active &= self.length.is_active();

                if v & 0x80 != 0 {
                    self.length.trigger(frame_step);
                    self.current_wave = 0;
                    self.delay = self.period + WAVE_INITIAL_DELAY;

                    if self.dac_enabled {
                        self.active = true;
                    }
                }
            }
            0xFF30..=0xFF3F => {
                let idx = a as usize - 0xFF30;
                if !self.active {
                    self.waveram[idx] = v;
                } else if !self.dmg_mode || self.sample_recently_accessed {
                    self.waveram[self.current_wave as usize >> 1] = v;
                }
            }
            _ => {}
        }
    }

    fn calculate_period(&mut self) {
        self.period = if self.frequency > 2048 {
            0
        } else {
            (2048 - self.frequency as u32) * 2
        };
    }

    fn on(&self) -> bool {
        self.active
    }

    fn run(&mut self, start_time: u32, end_time: u32) {
        self.sample_recently_accessed = false;
        if !self.active || self.period == 0 {
            if self.last_amp != 0 {
                self.blip.add_delta(start_time, -self.last_amp);
                self.last_amp = 0;
                self.delay = 0;
            }
            return;
        }

        let mut time = start_time + self.delay;
        let volshift = match self.volume_shift {
            0 => 4 + 2,
            1 => 0,
            2 => 1,
            3 => 2,
            _ => 0,
        };

        while time < end_time {
            let wavebyte = self.waveram[self.current_wave as usize >> 1];
            let sample = if self.current_wave % 2 == 0 {
                wavebyte >> 4
            } else {
                wavebyte & 0xF
            };

            let amp = ((sample << 2) >> volshift) as i32;
            if amp != self.last_amp {
                self.blip.add_delta(time, amp - self.last_amp);
                self.last_amp = amp;
            }

            if time >= end_time.saturating_sub(2) {
                self.sample_recently_accessed = true;
            }
            time += self.period;
            self.current_wave = (self.current_wave + 1) % 32;
        }

        self.delay = time - end_time;
    }

    fn step_length(&mut self) {
        self.length.step();
        self.active &= self.length.is_active();
    }
}

struct NoiseChannel {
    active: bool,
    dac_enabled: bool,
    reg_ff22: u8,
    length: LengthCounter,
    volume_envelope: VolumeEnvelope,
    period: u32,
    shift_width: u8,
    state: u16,
    delay: u32,
    last_amp: i32,
    blip: BlipBuf,
}

impl NoiseChannel {
    fn new(sample_rate: u32) -> Self {
        let mut blip = BlipBuf::new((OUTPUT_SAMPLE_COUNT + 1) as u32);
        blip.set_rates(CLOCKS_PER_SECOND as f64, sample_rate as f64);

        Self {
            active: false,
            dac_enabled: false,
            reg_ff22: 0,
            length: LengthCounter::new(64),
            volume_envelope: VolumeEnvelope::new(),
            period: 2048,
            shift_width: 14,
            state: 1,
            delay: 0,
            last_amp: 0,
            blip,
        }
    }

    fn rb(&self, a: u16) -> u8 {
        match a {
            0xFF20 => 0xFF,
            0xFF21 => self.volume_envelope.rb(a),
            0xFF22 => self.reg_ff22,
            0xFF23 => 0x80 | (if self.length.enabled { 0x40 } else { 0 }) | 0x3F,
            _ => 0xFF,
        }
    }

    fn wb(&mut self, a: u16, v: u8, frame_step: u8) {
        match a {
            0xFF20 => self.length.set(v & 0x3F),
            0xFF21 => {
                self.dac_enabled = v & 0xF8 != 0;
                self.active = self.active && self.dac_enabled;
            }
            0xFF22 => {
                self.reg_ff22 = v;
                self.shift_width = if v & 8 != 0 { 6 } else { 14 };
                let freq_div = match v & 7 {
                    0 => 8,
                    n => n as u32 * 16,
                };
                self.period = freq_div << (v >> 4);
            }
            0xFF23 => {
                self.length.enable(v & 0x40 != 0, frame_step);
                self.active &= self.length.is_active();

                if v & 0x80 != 0 {
                    self.length.trigger(frame_step);
                    self.state = 0xFF;
                    self.delay = 0;

                    if self.dac_enabled {
                        self.active = true;
                    }
                }
            }
            _ => {}
        }
        self.volume_envelope.wb(a, v);
    }

    fn on(&self) -> bool {
        self.active
    }

    fn run(&mut self, start_time: u32, end_time: u32) {
        if !self.active {
            if self.last_amp != 0 {
                self.blip.add_delta(start_time, -self.last_amp);
                self.last_amp = 0;
                self.delay = 0;
            }
            return;
        }

        let mut time = start_time + self.delay;
        while time < end_time {
            let oldstate = self.state;
            self.state <<= 1;
            let bit = ((oldstate >> self.shift_width) ^ (self.state >> self.shift_width)) & 1;
            self.state |= bit;

            let amp = match (oldstate >> self.shift_width) & 1 {
                0 => -(self.volume_envelope.volume as i32),
                _ => self.volume_envelope.volume as i32,
            };

            if self.last_amp != amp {
                self.blip.add_delta(time, amp - self.last_amp);
                self.last_amp = amp;
            }
            time += self.period;
        }
        self.delay = time - end_time;
    }

    fn step_length(&mut self) {
        self.length.step();
        self.active &= self.length.is_active();
    }
}

/// Sound processor
pub struct Sound {
    on: bool,
    time: u32,
    prev_time: u32,
    next_time: u32,
    frame_step: u8,
    output_period: u32,
    channel1: SquareChannel,
    channel2: SquareChannel,
    channel3: WaveChannel,
    channel4: NoiseChannel,
    volume_left: u8,
    volume_right: u8,
    reg_vin_to_so: u8,
    reg_ff25: u8,
    need_sync: bool,
    dmg_mode: bool,
    player: Box<dyn AudioOutput>,
}

impl Sound {
    /// Create sound processor for DMG
    pub fn new_dmg(player: Box<dyn AudioOutput>) -> Self {
        Self::new_internal(player, true)
    }

    /// Create sound processor for CGB
    pub fn new_cgb(player: Box<dyn AudioOutput>) -> Self {
        Self::new_internal(player, false)
    }

    /// Enable sound output
    pub fn set_on(&mut self) {
        self.on = true;
    }

    fn new_internal(player: Box<dyn AudioOutput>, dmg_mode: bool) -> Self {
        let sample_rate = player.sample_rate();
        let output_period =
            (OUTPUT_SAMPLE_COUNT as u64 * CLOCKS_PER_SECOND as u64) / sample_rate as u64;

        Self {
            on: false,
            time: 0,
            prev_time: 0,
            next_time: CLOCKS_PER_FRAME,
            frame_step: 0,
            output_period: output_period as u32,
            channel1: SquareChannel::new(sample_rate, true),
            channel2: SquareChannel::new(sample_rate, false),
            channel3: WaveChannel::new(sample_rate, dmg_mode),
            channel4: NoiseChannel::new(sample_rate),
            volume_left: 7,
            volume_right: 7,
            reg_vin_to_so: 0x00,
            reg_ff25: 0x00,
            need_sync: false,
            dmg_mode,
            player,
        }
    }

    /// Read a sound register
    pub fn rb(&mut self, a: u16) -> u8 {
        self.run();
        match a {
            0xFF10..=0xFF14 => self.channel1.rb(a),
            0xFF16..=0xFF19 => self.channel2.rb(a),
            0xFF1A..=0xFF1E => self.channel3.rb(a),
            0xFF20..=0xFF23 => self.channel4.rb(a),
            0xFF24 => ((self.volume_right & 7) << 4) | (self.volume_left & 7) | self.reg_vin_to_so,
            0xFF25 => self.reg_ff25,
            0xFF26 => {
                (if self.on { 0x80 } else { 0x00 })
                    | 0x70
                    | (if self.channel4.on() { 0x8 } else { 0x0 })
                    | (if self.channel3.on() { 0x4 } else { 0x0 })
                    | (if self.channel2.on() { 0x2 } else { 0x0 })
                    | (if self.channel1.on() { 0x1 } else { 0x0 })
            }
            0xFF30..=0xFF3F => self.channel3.rb(a),
            _ => 0xFF,
        }
    }

    /// Write to a sound register
    pub fn wb(&mut self, a: u16, v: u8) {
        if !self.on {
            if self.dmg_mode {
                match a {
                    0xFF11 => self.channel1.wb(a, v & 0x3F, self.frame_step),
                    0xFF16 => self.channel2.wb(a, v & 0x3F, self.frame_step),
                    0xFF1B => self.channel3.wb(a, v, self.frame_step),
                    0xFF20 => self.channel4.wb(a, v & 0x3F, self.frame_step),
                    _ => {}
                }
            }
            if a != 0xFF26 {
                return;
            }
        }

        self.run();
        match a {
            0xFF10..=0xFF14 => self.channel1.wb(a, v, self.frame_step),
            0xFF16..=0xFF19 => self.channel2.wb(a, v, self.frame_step),
            0xFF1A..=0xFF1E => self.channel3.wb(a, v, self.frame_step),
            0xFF20..=0xFF23 => self.channel4.wb(a, v, self.frame_step),
            0xFF24 => {
                self.volume_left = v & 0x7;
                self.volume_right = (v >> 4) & 0x7;
                self.reg_vin_to_so = v & 0x88;
            }
            0xFF25 => self.reg_ff25 = v,
            0xFF26 => {
                let turn_on = v & 0x80 != 0;
                if self.on && !turn_on {
                    for i in 0xFF10..=0xFF25 {
                        self.wb(i, 0);
                    }
                }
                if !self.on && turn_on {
                    self.frame_step = 0;
                }
                self.on = turn_on;
            }
            0xFF30..=0xFF3F => self.channel3.wb(a, v, self.frame_step),
            _ => {}
        }
    }

    /// Process audio for the given number of cycles
    pub fn do_cycle(&mut self, cycles: u32) {
        if !self.on {
            return;
        }

        self.time += cycles;
        if self.time >= self.output_period {
            self.do_output();
        }
    }

    /// Request audio sync
    pub fn sync(&mut self) {
        self.need_sync = true;
    }

    fn do_output(&mut self) {
        self.run();

        self.channel1.blip.end_frame(self.time);
        self.channel2.blip.end_frame(self.time);
        self.channel3.blip.end_frame(self.time);
        self.channel4.blip.end_frame(self.time);

        self.next_time -= self.time;
        self.time = 0;
        self.prev_time = 0;

        if !self.need_sync || self.player.underflowed() {
            self.need_sync = false;
            self.mix_buffers();
        } else {
            self.clear_buffers();
        }
    }

    fn run(&mut self) {
        while self.next_time <= self.time {
            self.channel1.run(self.prev_time, self.next_time);
            self.channel2.run(self.prev_time, self.next_time);
            self.channel3.run(self.prev_time, self.next_time);
            self.channel4.run(self.prev_time, self.next_time);

            if self.frame_step % 2 == 0 {
                self.channel1.step_length();
                self.channel2.step_length();
                self.channel3.step_length();
                self.channel4.step_length();
            }
            if self.frame_step % 4 == 2 {
                self.channel1.step_sweep();
            }
            if self.frame_step == 7 {
                self.channel1.volume_envelope.step();
                self.channel2.volume_envelope.step();
                self.channel4.volume_envelope.step();
            }

            self.frame_step = (self.frame_step + 1) % 8;
            self.prev_time = self.next_time;
            self.next_time += CLOCKS_PER_FRAME;
        }

        if self.prev_time != self.time {
            self.channel1.run(self.prev_time, self.time);
            self.channel2.run(self.prev_time, self.time);
            self.channel3.run(self.prev_time, self.time);
            self.channel4.run(self.prev_time, self.time);
            self.prev_time = self.time;
        }
    }

    fn mix_buffers(&mut self) {
        let sample_count = self.channel1.blip.samples_avail() as usize;
        let mut outputted = 0;

        let left_vol = (self.volume_left as f32 / 7.0) * (1.0 / 15.0) * 0.25;
        let right_vol = (self.volume_right as f32 / 7.0) * (1.0 / 15.0) * 0.25;

        while outputted < sample_count {
            let mut buf_left = [0f32; OUTPUT_SAMPLE_COUNT + 10];
            let mut buf_right = [0f32; OUTPUT_SAMPLE_COUNT + 10];
            let mut buf = [0i16; OUTPUT_SAMPLE_COUNT + 10];

            let count1 = self.channel1.blip.read_samples(&mut buf, false);
            for (i, v) in buf[..count1].iter().enumerate() {
                let sample = *v as f32;
                if self.reg_ff25 & 0x10 != 0 {
                    buf_left[i] += sample * left_vol;
                }
                if self.reg_ff25 & 0x01 != 0 {
                    buf_right[i] += sample * right_vol;
                }
            }

            let count2 = self.channel2.blip.read_samples(&mut buf, false);
            for (i, v) in buf[..count2].iter().enumerate() {
                let sample = *v as f32;
                if self.reg_ff25 & 0x20 != 0 {
                    buf_left[i] += sample * left_vol;
                }
                if self.reg_ff25 & 0x02 != 0 {
                    buf_right[i] += sample * right_vol;
                }
            }

            let count3 = self.channel3.blip.read_samples(&mut buf, false);
            for (i, v) in buf[..count3].iter().enumerate() {
                let sample = (*v as f32) / 4.0;
                if self.reg_ff25 & 0x40 != 0 {
                    buf_left[i] += sample * left_vol;
                }
                if self.reg_ff25 & 0x04 != 0 {
                    buf_right[i] += sample * right_vol;
                }
            }

            let count4 = self.channel4.blip.read_samples(&mut buf, false);
            for (i, v) in buf[..count4].iter().enumerate() {
                let sample = *v as f32;
                if self.reg_ff25 & 0x80 != 0 {
                    buf_left[i] += sample * left_vol;
                }
                if self.reg_ff25 & 0x08 != 0 {
                    buf_right[i] += sample * right_vol;
                }
            }

            self.player.play(&buf_left[..count1], &buf_right[..count1]);
            outputted += count1;
        }
    }

    fn clear_buffers(&mut self) {
        self.channel1.blip.clear();
        self.channel2.blip.clear();
        self.channel3.blip.clear();
        self.channel4.blip.clear();
    }

    /// Serialize sound state
    pub fn serialize(&self, _output: &mut Vec<u8>) {
        // Audio state serialization - simplified for now
    }

    /// Deserialize sound state  
    pub fn deserialize(&mut self, _data: &[u8]) -> Result<usize, ()> {
        Ok(0)
    }
}

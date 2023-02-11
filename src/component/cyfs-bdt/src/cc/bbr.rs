use std::{
    fmt::Debug, 
    time::Duration
};

use cyfs_base::*;
use crate::types::*;
use super::cc_impl::CcImpl;


#[derive(Debug, Copy, Clone, Default)]
struct MinMaxSample {
    time: u64,
    val: u64,
}

#[derive(Copy, Clone, Debug)]
struct MinMax {
    window: u64,
    samples: [MinMaxSample; 3],
}

impl MinMax {
    fn new(window: u64) -> Self {
        MinMax {
            window,
            samples: [Default::default(); 3],
        }
    }

    fn get(&self) -> u64 {
        self.samples[0].val
    }

    fn reset(&mut self) {
        self.samples.fill(Default::default());
    }

    fn update_max(&mut self, time: u64, meas: u64) {
        let sample = MinMaxSample {
            time,
            val: meas,
        };

        if self.samples[0].val == 0 
            || sample.val >= self.samples[0].val
            || sample.time - self.samples[2].time > self.window
        {
            self.samples.fill(sample);
            return;
        }

        if sample.val >= self.samples[1].val {
            self.samples[2] = sample;
            self.samples[1] = sample;
        } else if sample.val >= self.samples[2].val {
            self.samples[2] = sample;
        }

        self.subwin_update(sample);
    }

    fn subwin_update(&mut self, sample: MinMaxSample) {
        let dt = sample.time - self.samples[0].time;
        if dt > self.window {
            self.samples[0] = self.samples[1];
            self.samples[1] = self.samples[2];
            self.samples[2] = sample;
            if sample.time - self.samples[0].time > self.window {
                self.samples[0] = self.samples[1];
                self.samples[1] = self.samples[2];
                self.samples[2] = sample;
            }
        } else if self.samples[1].time == self.samples[0].time && dt > self.window / 4 {
            self.samples[2] = sample;
            self.samples[1] = sample;
        } else if self.samples[2].time == self.samples[1].time && dt > self.window / 2 {
            self.samples[2] = sample;
        }
    }
}



#[derive(Clone)]
struct BandwidthEstimation {
    total_acked: u64,
    prev_total_acked: u64,
    acked_time: Timestamp,
    prev_acked_time: Timestamp,
    total_sent: u64,
    prev_total_sent: u64,
    sent_time: Timestamp,
    prev_sent_time: Timestamp,
    max_filter: MinMax,
    acked_at_last_window: u64,

    bw_info_show_time: Timestamp,
}

impl Default for BandwidthEstimation {
    fn default() -> Self {
        BandwidthEstimation {
            total_acked: 0,
            prev_total_acked: 0,
            acked_time: 0,
            prev_acked_time: 0,
            total_sent: 0,
            prev_total_sent: 0,
            sent_time: 0,
            prev_sent_time: 0,
            max_filter: MinMax::new(10),
            acked_at_last_window: 0,
            bw_info_show_time: bucky_time_now(),
        }
    }
}

impl Debug for BandwidthEstimation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.2} KB/s",
            self.get_estimate() as f32 / 1024 as f32
        )
    }
}

impl BandwidthEstimation {
    // pub fn bw_info(&mut self) -> bool {
    //     let now = SystemTime::now();
    //     if system_time_to_bucky_time(&now) > self.bw_info_show_time {
    //         info!("bbr-BandwidthEstimation-BW: {:?}", self);
    //         self.bw_info_show_time = system_time_to_bucky_time(&(now + Duration::from_secs(5)));
    //         true
    //     } else {
    //         false
    //     }
    // }

    fn on_sent(&mut self, now: Timestamp, bytes: u64) {
        self.prev_total_sent = self.total_sent;
        self.total_sent += bytes;
        self.prev_sent_time = self.sent_time;
        self.sent_time = now;
    }

    fn on_ack(
        &mut self,
        now: Timestamp,
        _sent: Timestamp,
        bytes: u64,
        round: u64,
        app_limited: bool,
    ) {
        self.prev_total_acked = self.total_acked;
        self.total_acked += bytes;
        self.prev_acked_time = self.acked_time;
        self.acked_time = now;

        if self.prev_sent_time == 0 {
            return;
        }

        let send_rate = if self.sent_time > self.prev_sent_time {
            Self::bw_from_delta(
                self.total_sent - self.prev_total_sent,
                Duration::from_micros(self.sent_time - self.prev_sent_time)
            )
        } else {
            u64::MAX
        };

        let ack_rate= if self.prev_acked_time == 0 {
            0
        } else {
            Self::bw_from_delta(
                self.total_acked - self.prev_total_acked,
                Duration::from_micros(self.acked_time - self.prev_acked_time)
            )
        };

        let bandwidth = send_rate.min(ack_rate);
        if !app_limited && self.max_filter.get() < bandwidth {
            self.max_filter.update_max(round, bandwidth);
        }
    }

    fn bytes_acked_this_window(&self) -> u64 {
        self.total_acked - self.acked_at_last_window
    }

    fn end_acks(&mut self, _current_round: u64, _app_limited: bool) {
        self.acked_at_last_window = self.total_acked;
    }

    fn get_estimate(&self) -> u64 {
        self.max_filter.get()
    }

    fn bw_from_delta(bytes: u64, delta: Duration) -> u64 {
        let window_duration_ns = delta.as_nanos();
        if window_duration_ns == 0 {
            return 0;
        }
        let b_ns = bytes * 1_000_000_000;
        let bytes_per_second = b_ns / (window_duration_ns as u64);
        bytes_per_second
    }
}





#[derive(Debug, Clone)]
pub struct Config {
    pub min_cwnd: u64, 
    pub init_cwnd: u64, 
    pub probe_rtt_time: Duration, 
    pub probe_rtt_based_on_bdp: bool, 
    pub drain_to_target: bool, 
    pub startup_growth_target: f32, 
    pub default_high_gain: f32, 
    pub derived_high_cwnd_gain: f32, 
    pub pacing_gain: [f32; 8], 
    pub min_rtt_expire_time: Duration, 
    pub mode_rate_probe_rtt_multiplier: f32, 
    pub round_trips_with_growth_before_exiting_startup: u8, 
}


impl Default for Config {
    fn default() -> Self {
        Self {
            min_cwnd: 2, 
            init_cwnd: 4, 
            probe_rtt_time: Duration::from_millis(200), 
            probe_rtt_based_on_bdp: true, 
            drain_to_target: true, 
            startup_growth_target: 1.25, 
            default_high_gain: 2.885, 
            derived_high_cwnd_gain: 2.0, 
            pacing_gain: [1.25, 0.75, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],  
            min_rtt_expire_time: Duration::from_secs(10), 
            mode_rate_probe_rtt_multiplier: 0.75, 
            round_trips_with_growth_before_exiting_startup: 3
        }
    }
}



#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Mode {
    Startup,
    Drain,
    ProbeBw,
    ProbeRtt,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum RecoveryState {
    NotInRecovery,
    Conservation,
    Growth,
}

impl RecoveryState {
    fn in_recovery(&self) -> bool {
        !matches!(self, RecoveryState::NotInRecovery)
    }
}

#[derive(Debug, Copy, Clone)]
struct AckAggregationState {
    max_ack_height: MinMax,
    aggregation_epoch_start_time: Timestamp,
    aggregation_epoch_bytes: u64,
}


impl AckAggregationState {
    fn new() -> Self {
        Self {
            max_ack_height: MinMax::new(10),
            aggregation_epoch_start_time: bucky_time_now(),
            aggregation_epoch_bytes: 0,
        }
    }

    fn update_ack_aggregation_bytes(
        &mut self,
        newly_acked_bytes: u64,
        now: Timestamp,
        round: u64,
        max_bandwidth: u64,
    ) -> u64 {
        let expected_bytes_acked = if now > self.aggregation_epoch_start_time {
            max_bandwidth * (now - self.aggregation_epoch_start_time) / 1_000_000
        } else {
            0
        };
            
        if self.aggregation_epoch_bytes <= expected_bytes_acked {
            self.aggregation_epoch_bytes = newly_acked_bytes;
            self.aggregation_epoch_start_time = now;
            return 0;
        }

        self.aggregation_epoch_bytes += newly_acked_bytes;
        let diff = self.aggregation_epoch_bytes - expected_bytes_acked;
        self.max_ack_height.update_max(round, diff);
        diff
    }
}

#[derive(Debug, Clone, Default)]
struct LossState {
    lost_bytes: u64,
}

impl LossState {
    fn reset(&mut self) {
        self.lost_bytes = 0;
    }

    fn has_losses(&self) -> bool {
        self.lost_bytes != 0
    }
}

#[derive(Debug)]
pub struct Bbr {
    config: Config,
    rtt: Duration,
    mss: u64, 

    cwnd: u64,
    max_bandwidth: BandwidthEstimation,
    acked_bytes: u64,
    mode: Mode,
    loss_state: LossState,
    recovery_state: RecoveryState,
    recovery_window: u64,
    is_at_full_bandwidth: bool,
    last_cycle_start: Option<Timestamp>,
    current_cycle_offset: u8,
    prev_in_flight_count: u64,
    exit_probe_rtt_at: Option<Timestamp>,
    probe_rtt_last_started_at: Option<Timestamp>,
    min_rtt: Duration,
    exiting_quiescence: bool,
    pacing_rate: u64,
    max_acked_packet_number: u64,
    max_sent_packet_number: u64,
    end_recovery_at_packet_number: u64,
    current_round_trip_end_packet_number: u64,
    round_count: u64,
    bw_at_last_round: u64,
    ack_aggregation: AckAggregationState,
    pacing_gain: f32,
    high_gain: f32,
    drain_gain: f32,
    cwnd_gain: f32,
    high_cwnd_gain: f32,
    round_wo_bw_gain: u64,
}

impl Bbr {
    pub fn new(mss: usize, config: &Config) -> Self {
        let mut config = config.clone();
        config.min_cwnd = config.min_cwnd * mss as u64;
        config.init_cwnd = config.init_cwnd * mss as u64;


        Self {
            cwnd: config.init_cwnd,
            max_bandwidth: BandwidthEstimation::default(),

            acked_bytes: 0,
            mode: Mode::Startup,
            loss_state: Default::default(),
            recovery_state: RecoveryState::NotInRecovery,
            recovery_window: 0,
            is_at_full_bandwidth: false,
            pacing_gain: config.default_high_gain,
            high_gain: config.default_high_gain,
            drain_gain: 1.0 / config.default_high_gain,
            cwnd_gain: config.default_high_gain,
            high_cwnd_gain: config.default_high_gain,
            last_cycle_start: None,
            current_cycle_offset: 0,
            prev_in_flight_count: 0,
            exit_probe_rtt_at: None,
            probe_rtt_last_started_at: None,
            min_rtt: Default::default(),
            exiting_quiescence: false,
            pacing_rate: 0,
            max_acked_packet_number: 0,
            max_sent_packet_number: 0,
            end_recovery_at_packet_number: 0,
            current_round_trip_end_packet_number: 0,
            round_count: 0,
            bw_at_last_round: 0,
            round_wo_bw_gain: 0,
            ack_aggregation: AckAggregationState::new(),

            mss: mss as u64, 
            rtt: Duration::from_secs(0),
            config, 
        }
    }

    fn enter_startup_mode(&mut self) {
        self.mode = Mode::Startup;
        self.pacing_gain = self.high_gain;
        self.cwnd_gain = self.high_cwnd_gain;
    }

    fn enter_probe_bandwidth_mode(&mut self, now: Timestamp) {
        self.mode = Mode::ProbeBw;
        self.cwnd_gain = self.config.derived_high_cwnd_gain;
        self.last_cycle_start = Some(now);
        
        let mut rand_index = rand::random::<u8>() % (self.config.pacing_gain.len() as u8 - 1);
        if rand_index >= 1 {
            rand_index += 1;
        }
        self.current_cycle_offset = rand_index;
        self.pacing_gain = self.config.pacing_gain[rand_index as usize];
    }

    fn update_recovery_state(&mut self, is_round_start: bool) {
        if self.loss_state.has_losses() {
            self.end_recovery_at_packet_number = self.max_sent_packet_number;
        }
        match self.recovery_state {
            RecoveryState::NotInRecovery if self.loss_state.has_losses() => {
                self.recovery_state = RecoveryState::Conservation;
                self.recovery_window = 0;
                self.current_round_trip_end_packet_number = self.max_sent_packet_number;
            }
            RecoveryState::Growth | RecoveryState::Conservation => {
                if self.recovery_state == RecoveryState::Conservation && is_round_start {
                    self.recovery_state = RecoveryState::Growth;
                }
                if !self.loss_state.has_losses()
                    && self.max_acked_packet_number > self.end_recovery_at_packet_number
                {
                    self.recovery_state = RecoveryState::NotInRecovery;
                }
            }
            _ => {}
        }
    }

    fn update_gain_cycle_phase(&mut self, now: Timestamp, in_flight: u64) {
        let mut should_advance_gain_cycling = self
            .last_cycle_start
            .map(|last_cycle_start| Duration::from_micros(now - last_cycle_start) > self.min_rtt)
            .unwrap_or(false);
        if self.pacing_gain > 1.0
            && !self.loss_state.has_losses()
            && self.prev_in_flight_count < self.get_target_cwnd(self.pacing_gain)
        {
            should_advance_gain_cycling = false;
        }

        if self.pacing_gain < 1.0 && in_flight <= self.get_target_cwnd(1.0) {
            should_advance_gain_cycling = true;
        }

        if should_advance_gain_cycling {
            self.current_cycle_offset = (self.current_cycle_offset + 1) % self.config.pacing_gain.len() as u8;
            self.last_cycle_start = Some(now);
            
            if self.config.drain_to_target
                && self.pacing_gain < 1.0
                && (self.config.pacing_gain[self.current_cycle_offset as usize] - 1.0).abs() < f32::EPSILON
                && in_flight > self.get_target_cwnd(1.0)
            {
                return;
            }
            self.pacing_gain = self.config.pacing_gain[self.current_cycle_offset as usize];
        }
    }

    fn maybe_exit_startup_or_drain(&mut self, now: Timestamp, in_flight: u64) {
        if self.mode == Mode::Startup && self.is_at_full_bandwidth {
            self.mode = Mode::Drain;
            self.pacing_gain = self.drain_gain;
            self.cwnd_gain = self.high_cwnd_gain;
        }
        if self.mode == Mode::Drain && in_flight <= self.get_target_cwnd(1.0) {
            self.enter_probe_bandwidth_mode(now);
        }
    }

    fn is_min_rtt_expired(&self, now: Timestamp, app_limited: bool) -> bool {
        !app_limited
            && self
                .probe_rtt_last_started_at
                .map(|last| if now > last { Duration::from_micros(now - last) > self.config.min_rtt_expire_time } else { false })
                .unwrap_or(true)
    }

    fn maybe_enter_or_exit_probe_rtt(
        &mut self,
        now: Timestamp,
        is_round_start: bool,
        bytes_in_flight: u64,
        app_limited: bool,
    ) {
        let min_rtt_expired = self.is_min_rtt_expired(now, app_limited);
        if min_rtt_expired && !self.exiting_quiescence && self.mode != Mode::ProbeRtt {
            self.mode = Mode::ProbeRtt;
            self.pacing_gain = 1.0;
            self.exit_probe_rtt_at = None;
            self.probe_rtt_last_started_at = Some(now);
        }

        if self.mode == Mode::ProbeRtt {
            if self.exit_probe_rtt_at.is_none() {
                if bytes_in_flight < self.get_probe_rtt_cwnd() + self.mss {
                    self.exit_probe_rtt_at = Some(now + self.config.probe_rtt_time.as_micros() as u64);
                }
            } else if is_round_start && now >= self.exit_probe_rtt_at.unwrap() {
                if !self.is_at_full_bandwidth {
                    self.enter_startup_mode();
                } else {
                    self.enter_probe_bandwidth_mode(now);
                }
            }
        }

        self.exiting_quiescence = false;
    }

    fn get_target_cwnd(&self, gain: f32) -> u64 {
        let bw = self.max_bandwidth.get_estimate();
        let bdp = self.min_rtt.as_micros() as u64 * bw;
        let bdpf = bdp as f64;
        let cwnd = ((gain as f64 * bdpf) / 1_000_000f64) as u64;
        if cwnd == 0 {
            self.config.init_cwnd
        } else {
            cwnd.max(self.config.min_cwnd)
        }
        
    }

    fn get_probe_rtt_cwnd(&self) -> u64 {
        if self.config.probe_rtt_based_on_bdp {
            self.get_target_cwnd(self.config.mode_rate_probe_rtt_multiplier)
        } else {
            self.config.min_cwnd
        }
    }

    fn calculate_pacing_rate(&mut self) {
        let bw = self.max_bandwidth.get_estimate();
        if bw == 0 {
            return;
        }
        let target_rate = (bw as f64 * self.pacing_gain as f64) as u64;
        if self.is_at_full_bandwidth {
            self.pacing_rate = target_rate;
            return;
        }

        if self.pacing_rate == 0 && self.min_rtt.as_nanos() != 0 {
            self.pacing_rate = BandwidthEstimation::bw_from_delta(self.config.init_cwnd, self.min_rtt);
            return;
        }

        if self.pacing_rate < target_rate {
            self.pacing_rate = target_rate;
        }
    }

    fn calculate_cwnd(&mut self, bytes_acked: u64, excess_acked: u64) {
        if self.mode == Mode::ProbeRtt {
            return;
        }
        let mut target_window = self.get_target_cwnd(self.cwnd_gain);
        if self.is_at_full_bandwidth {
            target_window += self.ack_aggregation.max_ack_height.get();
        } else {
            target_window += excess_acked;
        }
        
        if self.is_at_full_bandwidth {
            self.cwnd = target_window.min(self.cwnd + bytes_acked);
        } else if (self.cwnd_gain < target_window as f32) || (self.acked_bytes < self.config.init_cwnd) {
            self.cwnd += bytes_acked;
        }

        self.cwnd = self.cwnd.max(self.config.min_cwnd);
    }

    fn calculate_recovery_window(&mut self, bytes_acked: u64, bytes_lost: u64, in_flight: u64) {
        if !self.recovery_state.in_recovery() {
            return;
        }
        
        if self.recovery_window == 0 {
            self.recovery_window = self.config.min_cwnd.max(in_flight + bytes_acked);
            return;
        }

        if self.recovery_window >= bytes_lost {
            self.recovery_window -= bytes_lost;
        } else {
            self.recovery_window = self.mss;
        }
        
        if self.recovery_state == RecoveryState::Growth {
            self.recovery_window += bytes_acked;
        }

        self.recovery_window = self.recovery_window.max(in_flight + bytes_acked).max(self.config.min_cwnd);
    }

    fn check_if_full_bw_reached(&mut self, app_limited: bool) {
        if app_limited {
            return;
        }
        let target = (self.bw_at_last_round as f64 * self.config.startup_growth_target as f64) as u64;
        let bw = self.max_bandwidth.get_estimate();
        if bw >= target {
            self.bw_at_last_round = bw;
            self.round_wo_bw_gain = 0;
            self.ack_aggregation.max_ack_height.reset();
            return;
        }

        self.round_wo_bw_gain += 1;
        if self.round_wo_bw_gain >= self.config.round_trips_with_growth_before_exiting_startup as u64
            || (self.recovery_state.in_recovery())
        {
            self.is_at_full_bandwidth = true;
        }
    }
}




impl CcImpl for Bbr {
    fn on_sent(&mut self, now: Timestamp, bytes: u64, last_packet_number: u64) {
        self.max_sent_packet_number = last_packet_number;
        self.max_bandwidth.on_sent(now, bytes);
    }

    fn cwnd(&self) -> u64 {
        if self.mode == Mode::ProbeRtt {
            self.get_probe_rtt_cwnd()
        } else if self.recovery_state.in_recovery()
            && self.mode != Mode::Startup {
            self.cwnd.min(self.recovery_window)
        } else {
            self.cwnd
        }
    }

    fn on_estimate(&mut self, rtt: Duration, _rto: Duration, _delay: Duration, app_limited: bool) {
        let now = bucky_time_now();

        if self.is_min_rtt_expired(now, app_limited) || self.min_rtt > rtt {
            self.min_rtt = rtt;
        }
    }

    fn on_ack(&mut self, flight: u64, ack: u64, largest_packet_num_acked: Option<u64>, sent_time: Timestamp, app_limited: bool) { //ret cwnd
        let now = bucky_time_now();

        self.max_bandwidth.on_ack(
            now,
            sent_time,
            ack,
            self.round_count,
            app_limited
        );
        self.acked_bytes += ack;

        let ack_in_wnd = self.max_bandwidth.bytes_acked_this_window();
        let excess_acked = self.ack_aggregation.update_ack_aggregation_bytes(
            ack_in_wnd,
            now,
            self.round_count,
            self.max_bandwidth.get_estimate(),
        );
        self.max_bandwidth.end_acks(self.round_count, app_limited);
        if let Some(largest_acked_packet) = largest_packet_num_acked {
            self.max_acked_packet_number = largest_acked_packet;
        }

        let mut is_round_start = false;
        if ack_in_wnd > 0 {
            is_round_start = self.max_acked_packet_number
                > self.current_round_trip_end_packet_number;
            if is_round_start {
                self.current_round_trip_end_packet_number =
                    self.max_sent_packet_number;
                self.round_count += 1;
            }
        }

        self.update_recovery_state(is_round_start);

        if self.mode == Mode::ProbeBw {
            self.update_gain_cycle_phase(now, flight);
        }

        if is_round_start && !self.is_at_full_bandwidth {
            self.check_if_full_bw_reached(app_limited);
        }

        self.maybe_exit_startup_or_drain(now, flight);

        self.maybe_enter_or_exit_probe_rtt(now, is_round_start, flight, app_limited);

        self.calculate_pacing_rate();
        self.calculate_cwnd(ack_in_wnd, excess_acked);
        self.calculate_recovery_window(
            ack_in_wnd,
            self.loss_state.lost_bytes,
            flight,
        );
        self.prev_in_flight_count = flight;
        self.loss_state.reset();
    }

    fn on_loss(&mut self, lost: u64) {
        self.loss_state.lost_bytes += lost;
    }

    fn on_no_resp(&mut self, rto: Duration, lost: u64) -> Duration {
        self.loss_state.lost_bytes += lost;
        rto
    }

    fn on_time_escape(&mut self, _: Timestamp) {
    }

    fn rate(&self) -> u64 {
        self.max_bandwidth.get_estimate()
    }
}

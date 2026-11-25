//! Continuous mood: valence (−1 sad … +1 happy) and energy (0 drained … 1 pumped).
//! Drifts slowly toward targets driven by the machine, the clock and interactions.

use crate::ai::personality::Axes;
use crate::watcher::LiveStats;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Mood {
    pub valence: f64,
    pub energy: f64,
    /// Short-lived boost/penalty from interactions, decays to 0.
    pub social: f64,
}

impl Default for Mood {
    fn default() -> Self {
        Self {
            valence: 0.3,
            energy: 0.6,
            social: 0.0,
        }
    }
}

pub struct MoodInputs<'a> {
    pub stats: &'a LiveStats,
    pub axes: &'a Axes,
    pub hour: u32,
    pub sleeping: bool,
    pub dizzy: bool,
    pub network_up: bool,
    pub recent_lag: bool,
}

impl Mood {
    pub fn new(axes: &Axes) -> Self {
        Self {
            valence: 0.25 + (0.5 - axes.anxiety) * 0.3,
            energy: 0.4 + axes.energy * 0.4,
            social: 0.0,
        }
    }

    /// Interaction nudge (pet = +, shake = −).
    pub fn nudge(&mut self, valence: f64, energy: f64) {
        self.social = (self.social + valence).clamp(-1.0, 1.0);
        self.energy = (self.energy + energy).clamp(0.0, 1.0);
    }

    pub fn update(&mut self, dt_secs: f64, inp: &MoodInputs) {
        let s = inp.stats;
        let a = inp.axes;

        // Circadian curve: low at 3-5h, peak late morning, dip after lunch.
        let circadian = match inp.hour {
            0..=4 => 0.15,
            5..=6 => 0.35,
            7..=8 => 0.6,
            9..=11 => 0.85,
            12..=13 => 0.65,
            14..=17 => 0.75,
            18..=20 => 0.6,
            21..=22 => 0.45,
            _ => 0.3,
        };
        let mut energy_target = 0.25 + a.energy * 0.35 + circadian * 0.4;
        if let (Some(p), Some(false)) = (s.battery_pct, s.on_ac) {
            if p < 30.0 {
                energy_target -= 0.25;
            }
        }
        if s.uptime_secs > 24 * 3600 {
            energy_target -= 0.1;
        }
        if inp.sleeping {
            // Sleeping recharges.
            energy_target = 0.95;
        }

        let mut valence_target = 0.3 + (0.5 - a.anxiety) * 0.3 + self.social * 0.5;
        if s.cpu_pct > 85.0 {
            valence_target -= 0.15 + a.anxiety * 0.2;
        }
        if s.ram_pct > 90.0 {
            valence_target -= 0.15;
        }
        if s.disk_pct > 92.0 {
            valence_target -= 0.2;
        }
        if s.cpu_temp.unwrap_or(0.0).max(s.gpu_temp.unwrap_or(0.0)) > 85.0 {
            valence_target -= 0.2;
        }
        if !inp.network_up {
            valence_target -= 0.15 + a.anxiety * 0.15;
        }
        if inp.recent_lag {
            valence_target -= 0.25;
        }
        if inp.dizzy {
            valence_target -= 0.05;
        }
        if s.gpu_pct > 70.0 && a.pride > 0.6 {
            // Proud machines love being pushed.
            valence_target += 0.15;
        }

        let k_e = 1.0 - (-dt_secs / 240.0).exp();
        let k_v = 1.0 - (-dt_secs / 150.0).exp();
        self.energy += (energy_target.clamp(0.0, 1.0) - self.energy) * k_e;
        self.valence += (valence_target.clamp(-1.0, 1.0) - self.valence) * k_v;
        self.social *= (-dt_secs / 300.0).exp();
    }

    /// Multiplier for walk speed.
    pub fn speed_factor(&self) -> f64 {
        0.6 + self.energy * 0.8
    }

    pub fn label(&self) -> &'static str {
        match (self.valence, self.energy) {
            (v, e) if v > 0.5 && e > 0.6 => "ravi et plein d'énergie",
            (v, _) if v > 0.5 => "de très bonne humeur",
            (v, e) if v < -0.3 && e < 0.35 => "fatigué et un peu morose",
            (v, _) if v < -0.3 => "un peu contrarié",
            (_, e) if e < 0.3 => "fatigué",
            (_, e) if e > 0.75 => "en pleine forme",
            _ => "tranquille",
        }
    }
}

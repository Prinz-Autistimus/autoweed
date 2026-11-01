//! Lichtzyklus: wird bei jedem Durchlauf aus Konfiguration und Uhrzeit berechnet,
//! damit nach einem Neustart sofort wieder der richtige Zustand gilt.

use autoweed_shared::{LightPhase, LightSchedule};
use chrono::{DateTime, Days, Utc};

const SECS_PER_DAY: i64 = 24 * 60 * 60;

/// Aktive Phase im Modus Photoperiod: ausgehend von der zuletzt gesetzten
/// Phase so lange weiterschalten, wie deren `duration_days` abgelaufen ist.
pub fn active_phase(schedule: &LightSchedule, now: DateTime<Utc>) -> Option<&LightPhase> {
    let LightSchedule::Photoperiod {
        phases,
        active_phase,
        phase_started_at,
        ..
    } = schedule
    else {
        return None;
    };
    let mut index = (*active_phase as usize).min(phases.len().checked_sub(1)?);
    let mut started = *phase_started_at;
    while let Some(days) = phases[index].duration_days {
        let Some(end) = started.checked_add_days(Days::new(days.into())) else {
            break;
        };
        if now < end || index + 1 >= phases.len() {
            break;
        }
        started = end;
        index += 1;
    }
    Some(&phases[index])
}

pub fn light_on(schedule: &LightSchedule, now: DateTime<Utc>) -> bool {
    let (start_minute, on_minutes) = match schedule {
        LightSchedule::Autoflower {
            start_minute_utc,
            on_minutes,
        } => (*start_minute_utc, *on_minutes),
        LightSchedule::Photoperiod {
            start_minute_utc, ..
        } => match active_phase(schedule, now) {
            Some(phase) => (*start_minute_utc, phase.on_minutes),
            None => return false,
        },
    };
    let secs_today = now.timestamp().rem_euclid(SECS_PER_DAY);
    let since_start = (secs_today - i64::from(start_minute) * 60).rem_euclid(SECS_PER_DAY);
    since_start < i64::from(on_minutes) * 60
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, day, hour, minute, 0)
            .unwrap()
    }

    fn phase(name: &str, hours: u16, days: Option<u32>) -> LightPhase {
        LightPhase {
            name: name.into(),
            on_minutes: hours * 60,
            duration_days: days,
        }
    }

    #[test]
    fn window_wraps_past_midnight() {
        // 18/6 ab 20:00: an bis 14:00 am Folgetag.
        let schedule = LightSchedule::Autoflower {
            start_minute_utc: 20 * 60,
            on_minutes: 18 * 60,
        };
        assert!(light_on(&schedule, at(1, 20, 0)));
        assert!(light_on(&schedule, at(2, 3, 0)));
        assert!(light_on(&schedule, at(2, 13, 59)));
        assert!(!light_on(&schedule, at(2, 14, 0)));
        assert!(!light_on(&schedule, at(2, 19, 59)));
    }

    #[test]
    fn zero_and_full_day() {
        let off = LightSchedule::Autoflower {
            start_minute_utc: 0,
            on_minutes: 0,
        };
        let on = LightSchedule::Autoflower {
            start_minute_utc: 600,
            on_minutes: 24 * 60,
        };
        for hour in 0..24 {
            assert!(!light_on(&off, at(1, hour, 30)));
            assert!(light_on(&on, at(1, hour, 30)));
        }
    }

    #[test]
    fn phases_advance_after_duration() {
        let schedule = LightSchedule::Photoperiod {
            start_minute_utc: 6 * 60,
            phases: vec![
                phase("Keimling", 18, Some(2)),
                phase("Wachstum", 18, None),
                phase("Blüte", 12, None),
            ],
            active_phase: 0,
            phase_started_at: at(1, 12, 0),
        };
        assert_eq!(
            active_phase(&schedule, at(3, 11, 59)).unwrap().name,
            "Keimling"
        );
        // Wachstum hat keine Dauer: bleibt, bis der Benutzer umschaltet.
        assert_eq!(
            active_phase(&schedule, at(3, 12, 0)).unwrap().name,
            "Wachstum"
        );
        assert_eq!(
            active_phase(&schedule, at(30, 0, 0)).unwrap().name,
            "Wachstum"
        );
        // 12/12 ab 06:00 → um 19:00 aus, 18/6 → noch an.
        let LightSchedule::Photoperiod { phases, .. } = &schedule else {
            unreachable!()
        };
        let flower = LightSchedule::Photoperiod {
            start_minute_utc: 6 * 60,
            phases: phases.clone(),
            active_phase: 2,
            phase_started_at: at(1, 0, 0),
        };
        assert!(light_on(&schedule, at(10, 19, 0)));
        assert!(!light_on(&flower, at(10, 19, 0)));
    }

    #[test]
    fn last_phase_stays_active() {
        let schedule = LightSchedule::Photoperiod {
            start_minute_utc: 0,
            phases: vec![phase("A", 18, Some(1)), phase("B", 12, Some(1))],
            active_phase: 0,
            phase_started_at: at(1, 0, 0),
        };
        assert_eq!(active_phase(&schedule, at(25, 0, 0)).unwrap().name, "B");
    }
}

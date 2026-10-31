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

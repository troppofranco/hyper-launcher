//! Automatismi dei server, eseguiti dall'host: riavvio giornaliero a un'ora
//! fissa (con avvisi in chat a 5 e 1 minuto) e backup ogni N ore. Qui solo
//! le decisioni, senza effetti: l'host le applica.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeDelta};

/// Cosa fare per il riavvio giornaliero a questo controllo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartAction {
    /// Avviso in chat: mancano (circa) questi minuti.
    Warn(u32),
    Restart,
}

/// Memoria del riavvio di un server (avvisi già dati, ultimo riavvio).
#[derive(Debug, Clone, Default)]
pub struct RestartState {
    warned: Vec<(NaiveDate, u32)>,
    last_restart: Option<NaiveDate>,
}

/// Ora `"HH:MM"` valida.
pub fn parse_time(at: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(at.trim(), "%H:%M").ok()
}

/// Entro quanto dopo l'ora prevista si riavvia ancora (controlli ogni 20 s,
/// host appena partito…).
const LATE: TimeDelta = TimeDelta::minutes(10);

/// Decide il passo del riavvio giornaliero alle `at` (ora locale) adesso.
pub fn restart_action(
    at: NaiveTime,
    now: NaiveDateTime,
    state: &mut RestartState,
) -> Option<RestartAction> {
    let today = now.date();
    let target = today.and_time(at);
    if now >= target {
        if now < target + LATE && state.last_restart != Some(today) {
            state.last_restart = Some(today);
            return Some(RestartAction::Restart);
        }
        return None;
    }
    // Avviso a 5 minuti e a 1 minuto (uno solo per soglia al giorno).
    let left = target - now;
    let minutes = if left <= TimeDelta::minutes(1) {
        1
    } else if left <= TimeDelta::minutes(5) {
        5
    } else {
        return None;
    };
    state.warned.retain(|(d, _)| *d == today);
    if state.warned.contains(&(today, minutes)) {
        return None;
    }
    state.warned.push((today, minutes));
    // Nel messaggio i minuti che mancano davvero, arrotondati in su.
    let real = u32::try_from((left.num_seconds() + 59) / 60).unwrap_or(minutes);
    Some(RestartAction::Warn(real.clamp(1, minutes)))
}

/// Serve un backup automatico? Ogni `hours` dall'ultimo backup, contando
/// dall'avvio del server se è più recente. Tempi in secondi Unix.
pub fn backup_due(hours: u32, last_backup: Option<u64>, started: u64, now: u64) -> bool {
    let since = last_backup.map_or(started, |last| last.max(started));
    hours > 0 && now >= since + u64::from(hours) * 3600
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(h: u32, m: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 20)
            .unwrap()
            .and_hms_opt(h, m, s)
            .unwrap()
    }

    #[test]
    fn warns_then_restarts_once_a_day() {
        let time = parse_time("05:00").unwrap();
        let mut state = RestartState::default();
        assert_eq!(restart_action(time, at(4, 50, 0), &mut state), None);
        assert_eq!(
            restart_action(time, at(4, 55, 0), &mut state),
            Some(RestartAction::Warn(5))
        );
        assert_eq!(restart_action(time, at(4, 56, 0), &mut state), None);
        assert_eq!(
            restart_action(time, at(4, 59, 10), &mut state),
            Some(RestartAction::Warn(1))
        );
        assert_eq!(restart_action(time, at(4, 59, 30), &mut state), None);
        // Impostato all'ultimo: l'avviso dice i minuti veri.
        let mut soon = RestartState::default();
        assert_eq!(
            restart_action(time, at(4, 57, 30), &mut soon),
            Some(RestartAction::Warn(3))
        );
        assert_eq!(
            restart_action(time, at(5, 0, 5), &mut state),
            Some(RestartAction::Restart)
        );
        assert_eq!(restart_action(time, at(5, 0, 25), &mut state), None, "once");
        // Troppo tardi (host acceso dopo): niente riavvio a sorpresa.
        let mut late = RestartState::default();
        assert_eq!(restart_action(time, at(9, 0, 0), &mut late), None);
        assert!(parse_time("25:00").is_none());
        assert!(parse_time("ciao").is_none());
    }

    #[test]
    fn backup_every_n_hours() {
        let h = 3600;
        assert!(!backup_due(6, None, 0, 5 * h));
        assert!(backup_due(6, None, 0, 6 * h));
        assert!(!backup_due(6, Some(10 * h), 0, 15 * h));
        assert!(backup_due(6, Some(10 * h), 0, 16 * h));
        assert!(!backup_due(0, None, 0, 100 * h));
    }
}

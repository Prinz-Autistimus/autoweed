import type { Actuator } from './generated/Actuator'
import type { ErrorKind } from './generated/ErrorKind'
import type { Event } from './generated/Event'

const MINUTES_PER_DAY = 24 * 60

export function formatNumber(value: number | null | undefined, digits = 1): string {
  if (value === null || value === undefined || Number.isNaN(value)) return '–'
  return value.toLocaleString('de-DE', {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  })
}

export function formatDateTime(iso: string): string {
  return new Date(iso).toLocaleString('de-DE', {
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

export function formatTime(iso: string): string {
  return new Date(iso).toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' })
}

/**
 * Das Board rechnet den Lichtzyklus in UTC. Angezeigt wird Ortszeit mit dem
 * heutigen Versatz; nach der Zeitumstellung verschiebt sich die Anzeige um 1 h.
 */
function offsetMinutes(): number {
  return new Date().getTimezoneOffset()
}

export function utcMinuteToLocalTime(minute: number): string {
  const local = (((minute - offsetMinutes()) % MINUTES_PER_DAY) + MINUTES_PER_DAY) % MINUTES_PER_DAY
  const h = Math.floor(local / 60)
  const m = local % 60
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`
}

export function localTimeToUtcMinute(time: string): number {
  const [h, m] = time.split(':').map(Number)
  return ((((h * 60 + m + offsetMinutes()) % MINUTES_PER_DAY) + MINUTES_PER_DAY) % MINUTES_PER_DAY)
}

/** Zeitraum „06:00–24:00“ in Ortszeit. */
export function lightWindow(startMinuteUtc: number, onMinutes: number): string {
  if (onMinutes <= 0) return 'immer aus'
  if (onMinutes >= MINUTES_PER_DAY) return 'immer an'
  const end = utcMinuteToLocalTime(startMinuteUtc + onMinutes)
  return `${utcMinuteToLocalTime(startMinuteUtc)}–${end === '00:00' ? '24:00' : end} Uhr`
}

export const actuatorName: Record<Actuator, string> = {
  heater: 'Heizung',
  light: 'Licht',
  pump: 'Pumpe',
}

export const errorName: Record<ErrorKind, string> = {
  climate_sensor: 'Temperatur-/Feuchtesensor',
  soil_sensor: 'Bodenfeuchtesensor',
  time_sync: 'Uhrzeit',
  bucket_empty: 'Eimer leer?',
  no_config: 'Keine Konfiguration',
  actuator: 'Aktor',
}

export function describeEvent(event: Event): string {
  switch (event.type) {
    case 'started':
      return `Board gestartet (v${event.version})`
    case 'switched':
      return `${actuatorName[event.actuator]} ${event.on ? 'an' : 'aus'}`
    case 'pump_run':
      return `Gegossen: ${event.secs} s (${event.reason === 'auto' ? 'automatisch' : 'manuell'})`
    case 'pump_locked':
      return 'Pumpe gesperrt (Eimer leer?)'
    case 'pump_unlocked':
      return 'Pumpe entsperrt'
    case 'error_raised':
      return `Fehler: ${event.message}`
    case 'error_cleared':
      return `Behoben: ${errorName[event.kind]}`
    case 'config_applied':
      return `Konfiguration v${event.version} übernommen`
    case 'override_set':
      return `${actuatorName[event.actuator]} manuell ${event.on ? 'an' : 'aus'} für ${event.minutes} min`
    case 'override_cleared':
      return `${actuatorName[event.actuator]}: Übersteuerung beendet`
  }
}

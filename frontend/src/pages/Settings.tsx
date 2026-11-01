import { useEffect, useRef, useState, type ReactNode } from 'react'
import { ApiFailure, api, put } from '../api'
import { localTimeToUtcMinute, utcMinuteToLocalTime } from '../format'
import type { BoardConfig } from '../generated/BoardConfig'
import type { ServerStatus } from '../generated/ServerStatus'
import type { LightPhase } from '../generated/LightPhase'
import type { LightSchedule } from '../generated/LightSchedule'

function NumberField({
  label,
  value,
  onChange,
  step = 1,
  min,
  max,
  unit,
}: {
  label: string
  value: number
  onChange: (v: number) => void
  step?: number
  min?: number
  max?: number
  unit?: string
}) {
  return (
    <label className="field">
      <span>{label}</span>
      <span className="row">
        <input
          type="number"
          value={Number.isNaN(value) ? '' : value}
          step={step}
          min={min}
          max={max}
          onChange={(e) => onChange(e.target.valueAsNumber)}
        />
        {unit && <span className="muted">{unit}</span>}
      </span>
    </label>
  )
}

function Card({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="card section">
      <h2>{title}</h2>
      {children}
    </section>
  )
}

const hours = (minutes: number) => minutes / 60
const minutes = (hours: number) => Math.round(hours * 60)

export default function Settings() {
  const [saved, setSaved] = useState<BoardConfig | null>(null)
  const [form, setForm] = useState<BoardConfig | null>(null)
  const [soilRaw, setSoilRaw] = useState<number | null>(null)
  const [errors, setErrors] = useState<string[]>([])
  const [notice, setNotice] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const fileInput = useRef<HTMLInputElement>(null)

  useEffect(() => {
    api<BoardConfig>('/api/config').then((c) => {
      setSaved(c)
      setForm(c)
    })
    const loadRaw = () =>
      api<ServerStatus>('/api/status')
        .then((s) => setSoilRaw(s.board?.sensors.soil_raw ?? null))
        .catch(() => {})
    loadRaw()
    const timer = window.setInterval(loadRaw, 5000)
    return () => window.clearInterval(timer)
  }, [])

  if (!form || !saved) return <p className="muted">Lade …</p>

  /** Ändert eine Kopie der Konfiguration. */
  const update = (change: (c: BoardConfig) => void) => {
    const next = structuredClone(form)
    change(next)
    setForm(next)
    setNotice(null)
  }
  const dirty = JSON.stringify(form) !== JSON.stringify(saved)

  async function save(config: BoardConfig) {
    setBusy(true)
    setErrors([])
    try {
      const result = await put<BoardConfig>('/api/config', config)
      setSaved(result)
      setForm(result)
      setNotice(`Gespeichert als v${result.version}, wird an das Board übertragen.`)
    } catch (err) {
      if (err instanceof ApiFailure) setErrors(err.details.length ? err.details : [err.message])
      else setErrors(['Speichern fehlgeschlagen'])
    } finally {
      setBusy(false)
    }
  }

  function exportConfig() {
    const blob = new Blob([JSON.stringify(saved, null, 2)], { type: 'application/json' })
    const a = document.createElement('a')
    a.href = URL.createObjectURL(blob)
    a.download = `autoweed-config-v${saved!.version}.json`
    a.click()
    URL.revokeObjectURL(a.href)
  }

  async function importConfig(file: File) {
    try {
      const imported = JSON.parse(await file.text()) as BoardConfig
      setForm({ ...imported, version: saved!.version })
      setNotice('Datei geladen. Prüfen und dann speichern.')
    } catch {
      setErrors(['Die Datei ist keine gültige Konfiguration'])
    }
  }

  const light = form.light
  const setLight = (schedule: LightSchedule) => update((c) => (c.light = schedule))

  function switchMode(mode: LightSchedule['mode']) {
    if (mode === light.mode) return
    if (mode === 'autoflower') {
      setLight({ mode, start_minute_utc: light.start_minute_utc, on_minutes: 18 * 60 })
    } else {
      setLight({
        mode,
        start_minute_utc: light.start_minute_utc,
        phases: [
          { name: 'Wachstum', on_minutes: 18 * 60, duration_days: null },
          { name: 'Blüte', on_minutes: 12 * 60, duration_days: null },
        ],
        active_phase: 0,
        phase_started_at: new Date().toISOString(),
      })
    }
  }

  function updatePhase(index: number, change: (p: LightPhase) => void) {
    update((c) => {
      if (c.light.mode === 'photoperiod') change(c.light.phases[index])
    })
  }

  return (
    <>
      <Card title="Temperatur">
        <div className="fields">
          <NumberField
            label="Zieltemperatur"
            unit="°C"
            step={0.5}
            value={form.heater.target_celsius}
            onChange={(v) => update((c) => (c.heater.target_celsius = v))}
          />
          <NumberField
            label="Hysterese ±"
            unit="°C"
            step={0.1}
            value={form.heater.hysteresis_celsius}
            onChange={(v) => update((c) => (c.heater.hysteresis_celsius = v))}
          />
        </div>
        <p className="muted small">
          Heizung an unter {form.heater.target_celsius - form.heater.hysteresis_celsius} °C, aus über{' '}
          {form.heater.target_celsius + form.heater.hysteresis_celsius} °C.
        </p>
      </Card>

      <Card title="Bewässerung">
        <div className="fields">
          <NumberField label="Ziel-Bodenfeuchte" unit="%" value={form.watering.target_percent} onChange={(v) => update((c) => (c.watering.target_percent = v))} />
          <NumberField label="Hysterese" unit="%" value={form.watering.hysteresis_percent} onChange={(v) => update((c) => (c.watering.hysteresis_percent = v))} />
          <NumberField label="Pumpenlaufzeit pro Gießen" unit="s" min={1} max={120} value={form.watering.pump_secs} onChange={(v) => update((c) => (c.watering.pump_secs = v))} />
          <NumberField label="Mindestabstand" unit="min" min={1} value={form.watering.min_interval_mins} onChange={(v) => update((c) => (c.watering.min_interval_mins = v))} />
          <NumberField label="Höchstens pro Tag" unit="s" value={form.watering.max_pump_secs_per_day} onChange={(v) => update((c) => (c.watering.max_pump_secs_per_day = v))} />
        </div>
        <h3>„Eimer leer“-Erkennung</h3>
        <div className="fields">
          <NumberField label="Gießvorgänge ohne Wirkung" min={1} value={form.watering.empty_bucket.doses} onChange={(v) => update((c) => (c.watering.empty_bucket.doses = v))} />
          <NumberField label="Mindestanstieg pro Gießen" unit="%" step={0.5} value={form.watering.empty_bucket.min_rise_percent} onChange={(v) => update((c) => (c.watering.empty_bucket.min_rise_percent = v))} />
        </div>
        <p className="muted small">
          Gegossen wird unter {form.watering.target_percent - form.watering.hysteresis_percent} %. Steigt die Feuchte nach{' '}
          {form.watering.empty_bucket.doses} Gießvorgängen in Folge jeweils um weniger als {form.watering.empty_bucket.min_rise_percent} %, wird die Pumpe gesperrt.
        </p>
      </Card>

      <Card title="Bodensensor kalibrieren">
        <p className="muted small">
          Sensor in trockene Erde (bzw. an die Luft) halten und „trocken“ übernehmen, dann in ein Glas Wasser und „nass“ übernehmen.
          Aktueller Rohwert: <strong>{soilRaw ?? '–'}</strong>
        </p>
        <div className="fields">
          <div className="stack">
            <NumberField label="Rohwert trocken (0 %)" value={form.soil_calibration.dry_raw} onChange={(v) => update((c) => (c.soil_calibration.dry_raw = v))} />
            <button disabled={soilRaw === null} onClick={() => update((c) => (c.soil_calibration.dry_raw = soilRaw!))}>
              Aktuellen Wert als trocken
            </button>
          </div>
          <div className="stack">
            <NumberField label="Rohwert nass (100 %)" value={form.soil_calibration.wet_raw} onChange={(v) => update((c) => (c.soil_calibration.wet_raw = v))} />
            <button disabled={soilRaw === null} onClick={() => update((c) => (c.soil_calibration.wet_raw = soilRaw!))}>
              Aktuellen Wert als nass
            </button>
          </div>
        </div>
      </Card>

      <Card title="Licht">
        <div className="row" style={{ marginBottom: '1rem' }}>
          <button className={light.mode === 'autoflower' ? 'primary' : ''} onClick={() => switchMode('autoflower')}>
            Autoflower
          </button>
          <button className={light.mode === 'photoperiod' ? 'primary' : ''} onClick={() => switchMode('photoperiod')}>
            Photoperiodisch
          </button>
        </div>
        <div className="fields">
          <label className="field">
            <span>Licht an um (Ortszeit)</span>
            <input
              type="time"
              value={utcMinuteToLocalTime(light.start_minute_utc)}
              onChange={(e) => e.target.value && setLight({ ...light, start_minute_utc: localTimeToUtcMinute(e.target.value) })}
            />
          </label>
          {light.mode === 'autoflower' && (
            <NumberField
              label="Stunden an pro Tag"
              unit="h"
              step={0.5}
              min={0}
              max={24}
              value={hours(light.on_minutes)}
              onChange={(v) => setLight({ ...light, on_minutes: minutes(v) })}
            />
          )}
        </div>

        {light.mode === 'photoperiod' && (
          <div className="phases stack section">
            {light.phases.map((phase, i) => (
              <div className="phase" key={i}>
                <label className="field">
                  <span>{i === light.active_phase ? '▶ Aktive Phase' : `Phase ${i + 1}`}</span>
                  <input value={phase.name} onChange={(e) => updatePhase(i, (p) => (p.name = e.target.value))} />
                </label>
                <NumberField label="Stunden an" unit="h" step={0.5} min={0} max={24} value={hours(phase.on_minutes)} onChange={(v) => updatePhase(i, (p) => (p.on_minutes = minutes(v)))} />
                <label className="field">
                  <span>Wechsel nach</span>
                  <span className="row">
                    <input
                      type="number"
                      min={1}
                      placeholder="manuell"
                      value={phase.duration_days ?? ''}
                      onChange={(e) => updatePhase(i, (p) => (p.duration_days = e.target.value ? e.target.valueAsNumber : null))}
                    />
                    <span className="muted">Tagen</span>
                  </span>
                </label>
                <div className="row">
                  {i !== light.active_phase && (
                    <button onClick={() => setLight({ ...light, active_phase: i, phase_started_at: new Date().toISOString() })}>
                      Jetzt starten
                    </button>
                  )}
                  {light.phases.length > 1 && (
                    <button
                      aria-label="Phase entfernen"
                      onClick={() =>
                        setLight({
                          ...light,
                          phases: light.phases.filter((_, j) => j !== i),
                          active_phase: Math.min(light.active_phase - (i < light.active_phase ? 1 : 0), light.phases.length - 2),
                        })
                      }
                    >
                      ✕
                    </button>
                  )}
                </div>
              </div>
            ))}
            <div className="row">
              <button onClick={() => setLight({ ...light, phases: [...light.phases, { name: 'Neue Phase', on_minutes: 12 * 60, duration_days: null }] })}>
                + Phase
              </button>
            </div>
            <p className="muted small">
              Die aktive Phase läuft seit {new Date(light.phase_started_at).toLocaleDateString('de-DE')}. Phasen mit „Wechsel nach“ gehen danach
              automatisch in die nächste über, leer = nur manuell.
            </p>
          </div>
        )}
        <p className="muted small">
          Das Board rechnet in UTC, damit jeder Tag genau 24 h hat. Nach der Zeitumstellung geht das Licht laut Uhr eine Stunde früher bzw. später an.
        </p>
      </Card>

      <Card title="Messung">
        <div className="fields">
          <NumberField label="Messintervall" unit="s" min={2} max={10} value={form.measure_interval_secs} onChange={(v) => update((c) => (c.measure_interval_secs = v))} />
        </div>
      </Card>

      <section className="card section stack">
        {errors.length > 0 && (
          <ul className="error-list">
            {errors.map((e) => (
              <li key={e}>{e}</li>
            ))}
          </ul>
        )}
        {notice && <p className="muted">{notice}</p>}
        <div className="row">
          <button className="primary" disabled={!dirty || busy} onClick={() => save(form)}>
            Speichern
          </button>
          <button disabled={!dirty || busy} onClick={() => setForm(saved)}>
            Verwerfen
          </button>
          <span style={{ flex: 1 }} />
          <button onClick={exportConfig}>Exportieren</button>
          <button onClick={() => fileInput.current?.click()}>Importieren</button>
          <input
            ref={fileInput}
            type="file"
            accept="application/json"
            hidden
            onChange={(e) => {
              const file = e.target.files?.[0]
              if (file) importConfig(file)
              e.target.value = ''
            }}
          />
        </div>
        <p className="muted small">Gespeicherte Version: v{saved.version}</p>
      </section>
    </>
  )
}

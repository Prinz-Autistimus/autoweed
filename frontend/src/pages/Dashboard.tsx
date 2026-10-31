import { useCallback, useEffect, useState } from 'react'
import { ApiFailure, api, post } from '../api'
import { actuatorName, errorName, formatDateTime, formatNumber, formatTime, lightWindow } from '../format'
import type { BoardConfig } from '../generated/BoardConfig'
import type { EcsStatus } from '../generated/EcsStatus'
import type { Override } from '../generated/Override'

const REFRESH_MS = 5000

function OnOff({ on }: { on: boolean }) {
  return <span className={`pill ${on ? 'on' : 'off'}`}>{on ? 'an' : 'aus'}</span>
}

function OverrideControls({
  actuator,
  current,
  onAction,
}: {
  actuator: 'heater' | 'light'
  current: Override | null
  onAction: (path: string, body: unknown) => void
}) {
  const [minutes, setMinutes] = useState(30)
  const path = `/api/override/${actuator}`
  return (
    <div className="stack small">
      {current && (
        <div className="muted">
          Manuell {current.on ? 'an' : 'aus'} bis {formatTime(current.until)} Uhr
        </div>
      )}
      <div className="row">
        <input
          type="number"
          min={1}
          max={1440}
          value={minutes}
          aria-label="Minuten"
          onChange={(e) => setMinutes(Number(e.target.value))}
        />
        <span className="muted">min</span>
        <button onClick={() => onAction(path, { on: true, minutes })}>An</button>
        <button onClick={() => onAction(path, { on: false, minutes })}>Aus</button>
        {current && <button onClick={() => onAction(path, { on: false, minutes: 0 })}>Automatik</button>}
      </div>
    </div>
  )
}

export default function Dashboard() {
  const [status, setStatus] = useState<EcsStatus | null>(null)
  const [config, setConfig] = useState<BoardConfig | null>(null)
  const [pumpSecs, setPumpSecs] = useState(10)
  const [message, setMessage] = useState<string | null>(null)

  const load = useCallback(() => {
    api<EcsStatus>('/api/status').then(setStatus).catch(() => {})
  }, [])

  useEffect(() => {
    load()
    api<BoardConfig>('/api/config')
      .then((c) => {
        setConfig(c)
        setPumpSecs(c.watering.pump_secs)
      })
      .catch(() => {})
    const timer = window.setInterval(load, REFRESH_MS)
    return () => window.clearInterval(timer)
  }, [load])

  const action = useCallback(
    async (path: string, body: unknown) => {
      setMessage(null)
      try {
        await post(path, body)
        window.setTimeout(load, 1500)
      } catch (err) {
        setMessage(err instanceof ApiFailure ? err.message : 'Aktion fehlgeschlagen')
      }
    },
    [load],
  )

  if (!status) return <p className="muted">Lade …</p>
  const board = status.board
  const s = board?.sensors
  const errors = board?.errors.filter((e) => e.kind !== 'no_config') ?? []

  return (
    <>
      {!status.board_reachable && (
        <div className="banner critical">
          <strong>⚠ Board nicht erreichbar</strong>
          Letzter Kontakt: {status.last_contact ? formatDateTime(status.last_contact) : 'noch nie'}. Das Board
          regelt mit der letzten Konfiguration weiter, die Werte unten können veraltet sein.
        </div>
      )}
      {errors.map((e) => (
        <div key={e.kind} className={`banner ${e.kind === 'bucket_empty' ? 'critical' : ''}`}>
          <strong>⚠ {errorName[e.kind]}</strong>
          {e.message} <span className="muted small">(seit {formatDateTime(e.since)})</span>
          {e.kind === 'bucket_empty' && (
            <div className="row" style={{ marginTop: '0.5rem' }}>
              <button className="primary" onClick={() => action('/api/pump/unlock', {})}>
                Eimer aufgefüllt – Pumpe entsperren
              </button>
            </div>
          )}
        </div>
      ))}
      {status.board_reachable && !status.config_synced && (
        <div className="banner">
          <strong>Konfiguration wird übertragen …</strong>
          ECS hat v{status.config_version}, das Board nutzt {board?.config_version ? `v${board.config_version}` : 'noch keine'}.
        </div>
      )}
      {message && (
        <div className="banner critical">
          <strong>Aktion fehlgeschlagen</strong>
          {message}
        </div>
      )}

      <div className="grid">
        <section className="card">
          <h2>Temperatur</h2>
          <div className="big">{formatNumber(s?.temperature_celsius)} °C</div>
          {config && (
            <div className="muted small">
              Ziel {formatNumber(config.heater.target_celsius)} ± {formatNumber(config.heater.hysteresis_celsius)} °C
            </div>
          )}
          <h3 className="row">
            Heizung <OnOff on={board?.actuators.heater ?? false} />
          </h3>
          <OverrideControls actuator="heater" current={board?.overrides.heater ?? null} onAction={action} />
        </section>

        <section className="card">
          <h2>Luftfeuchtigkeit</h2>
          <div className="big">{formatNumber(s?.humidity_percent)} %</div>
          <div className="muted small">Nur Überwachung, keine Regelung</div>
        </section>

        <section className="card">
          <h2>Bodenfeuchte</h2>
          <div className="big">{formatNumber(s?.soil_percent)} %</div>
          {config && (
            <div className="muted small">
              Gießen unter {formatNumber(config.watering.target_percent - config.watering.hysteresis_percent)} %
              · heute {board?.pumped_secs_today ?? 0} / {config.watering.max_pump_secs_per_day} s
            </div>
          )}
          <h3 className="row">
            Pumpe <OnOff on={board?.actuators.pump ?? false} />
            {board?.pump_locked && <span className="pill">gesperrt</span>}
          </h3>
          <div className="row small">
            <input
              type="number"
              min={1}
              max={120}
              value={pumpSecs}
              aria-label="Sekunden"
              onChange={(e) => setPumpSecs(Number(e.target.value))}
            />
            <span className="muted">s</span>
            <button onClick={() => action('/api/override/pump', { secs: pumpSecs })}>Jetzt gießen</button>
          </div>
        </section>

        <section className="card">
          <h2>Licht</h2>
          <div className="big row">
            <OnOff on={board?.actuators.light ?? false} />
          </div>
          {config && (
            <div className="muted small">
              {config.light.mode === 'autoflower'
                ? `Autoflower: ${lightWindow(config.light.start_minute_utc, config.light.on_minutes)}`
                : `Phase ${board?.light_phase ?? '–'}: ${lightWindow(
                    config.light.start_minute_utc,
                    config.light.phases.find((p) => p.name === board?.light_phase)?.on_minutes ?? 0,
                  )}`}
            </div>
          )}
          <h3>Übersteuern</h3>
          <OverrideControls actuator="light" current={board?.overrides.light ?? null} onAction={action} />
        </section>
      </div>

      <section className="card section small muted">
        <div className="row">
          <span>Board {board ? `v${board.version}` : ''}</span>·
          <span>{status.board_reachable ? 'erreichbar' : 'nicht erreichbar'}</span>·
          <span>Konfiguration v{status.config_version} {status.config_synced ? '(übernommen)' : '(ausstehend)'}</span>·
          <span>Uhrzeit {board?.time_synced ? 'synchron' : 'nicht synchron'}</span>
          {status.last_contact && (
            <>
              ·<span>Stand {formatTime(status.last_contact)} Uhr</span>
            </>
          )}
        </div>
        {board?.overrides && (board.overrides.heater || board.overrides.light) && (
          <div>
            Übersteuert:{' '}
            {(['heater', 'light'] as const)
              .filter((a) => board.overrides[a])
              .map((a) => actuatorName[a])
              .join(', ')}
          </div>
        )}
      </section>
    </>
  )
}

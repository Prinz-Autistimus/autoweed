import { useEffect, useMemo, useState } from 'react'
import { api } from '../api'
import LineChart, { type Series } from '../components/LineChart'
import { describeEvent, formatDateTime, formatNumber } from '../format'
import type { BoardConfig } from '../generated/BoardConfig'
import type { Event } from '../generated/Event'
import type { History as HistoryData } from '../generated/History'
import type { HistoryPoint } from '../generated/HistoryPoint'

const HOUR = 3_600_000
const RANGES = [
  { key: '6h', label: '6 Stunden', ms: 6 * HOUR },
  { key: '24h', label: '24 Stunden', ms: 24 * HOUR },
  { key: '7d', label: '7 Tage', ms: 7 * 24 * HOUR },
  { key: '30d', label: '30 Tage', ms: 30 * 24 * HOUR },
  { key: '90d', label: '90 Tage', ms: 90 * 24 * HOUR },
] as const

type RangeKey = (typeof RANGES)[number]['key']

function series(points: HistoryPoint[], pick: (p: HistoryPoint) => number | null): Series {
  return {
    times: points.map((p) => Date.parse(p.at)),
    values: points.map(pick),
  }
}

/** Anteil „an“ (0–1) als Prozent. */
const percent = (v: number | null) => (v === null ? null : v * 100)

export default function History() {
  const [range, setRange] = useState<RangeKey>('24h')
  const [data, setData] = useState<HistoryData | null>(null)
  const [shownRange, setShownRange] = useState<[number, number]>([0, 1])
  const [events, setEvents] = useState<Event[]>([])
  const [config, setConfig] = useState<BoardConfig | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [hover, setHover] = useState<number | null>(null)
  const [table, setTable] = useState(false)

  useEffect(() => {
    api<BoardConfig>('/api/config').then(setConfig).catch(() => {})
  }, [])

  useEffect(() => {
    const ms = RANGES.find((r) => r.key === range)!.ms
    const end = new Date()
    const start = new Date(end.getTime() - ms)
    const query = `start=${start.toISOString()}&end=${end.toISOString()}`
    Promise.all([api<HistoryData>(`/api/history?${query}`), api<Event[]>(`/api/events?${query}`)])
      .then(([history, events]) => {
        setData(history)
        setShownRange([start.getTime(), end.getTime()])
        setEvents(events)
        setHover(null)
        setError(null)
      })
      .catch((err: Error) => setError(err.message))
  }, [range])

  const points = useMemo(() => data?.points ?? [], [data])
  const charts = useMemo(
    () => ({
      temperature: series(points, (p) => p.temperature_celsius),
      humidity: series(points, (p) => p.humidity_percent),
      soil: series(points, (p) => p.soil_percent),
      heater: series(points, (p) => percent(p.heater)),
      light: series(points, (p) => percent(p.light)),
      pump: series(points, (p) => percent(p.pump)),
    }),
    [points],
  )

  const common = { hover, onHover: setHover, range: shownRange }

  return (
    <>
      <div className="row" style={{ marginBottom: '1rem' }}>
        {RANGES.map((r) => (
          <button key={r.key} className={r.key === range ? 'primary' : ''} onClick={() => setRange(r.key)}>
            {r.label}
          </button>
        ))}
        <span style={{ flex: 1 }} />
        <button onClick={() => setTable(!table)}>{table ? 'Diagramme' : 'Tabelle'}</button>
      </div>
      {error && (
        <div className="banner critical">
          <strong>Verlauf konnte nicht geladen werden</strong>
          {error}
        </div>
      )}

      {data && !table && (
        <div className="stack">
          <section className="card stack">
            <LineChart
              title="Temperatur"
              unit="°C"
              color="var(--series-temp)"
              series={charts.temperature}
              target={config?.heater.target_celsius}
              {...common}
            />
            <LineChart title="Luftfeuchtigkeit" unit="%" color="var(--series-hum)" series={charts.humidity} {...common} />
            <LineChart
              title="Bodenfeuchte"
              unit="%"
              color="var(--series-soil)"
              series={charts.soil}
              target={config ? config.watering.target_percent - config.watering.hysteresis_percent : undefined}
              {...common}
            />
            <p className="muted small" style={{ margin: 0 }}>
              Gestrichelt: Zieltemperatur bzw. Gießschwelle. Werte gemittelt über {data.window_secs} s.
            </p>
          </section>
          <section className="card stack">
            <LineChart title="Licht an" unit="%" color="var(--series-act)" series={charts.light} domain={[0, 100]} digits={0} step {...common} />
            <LineChart title="Heizung an" unit="%" color="var(--series-act)" series={charts.heater} domain={[0, 100]} digits={0} step {...common} />
            <LineChart title="Pumpe an" unit="%" color="var(--series-act)" series={charts.pump} domain={[0, 100]} digits={0} step {...common} />
            <p className="muted small" style={{ margin: 0 }}>
              Anteil der Zeit, in der der Aktor im jeweiligen Zeitfenster eingeschaltet war.
            </p>
          </section>
        </div>
      )}

      {data && table && (
        <section className="card table-wrap">
          <table className="data">
            <thead>
              <tr>
                <th>Zeit</th>
                <th>Temperatur °C</th>
                <th>Luftfeuchte %</th>
                <th>Boden %</th>
                <th>Licht %</th>
                <th>Heizung %</th>
                <th>Pumpe %</th>
              </tr>
            </thead>
            <tbody>
              {[...points].reverse().map((p) => (
                <tr key={p.at}>
                  <td>{formatDateTime(p.at)}</td>
                  <td>{formatNumber(p.temperature_celsius)}</td>
                  <td>{formatNumber(p.humidity_percent)}</td>
                  <td>{formatNumber(p.soil_percent)}</td>
                  <td>{formatNumber(percent(p.light), 0)}</td>
                  <td>{formatNumber(percent(p.heater), 0)}</td>
                  <td>{formatNumber(percent(p.pump), 0)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      <section className="card section">
        <h2>Ereignisse</h2>
        {events.length === 0 ? (
          <p className="muted">Keine Ereignisse in diesem Zeitraum.</p>
        ) : (
          <ul className="events">
            {events.map((e, i) => (
              <li key={`${e.at}-${i}`}>
                <time dateTime={e.at}>{formatDateTime(e.at)}</time>
                <span>{describeEvent(e)}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </>
  )
}

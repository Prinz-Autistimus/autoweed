import { useEffect, useRef, useState, type PointerEvent } from 'react'

export interface Series {
  times: number[]
  values: (number | null)[]
}

const HEIGHT = 150
const PAD = { top: 8, right: 8, bottom: 22, left: 40 }

function useWidth<T extends HTMLElement>() {
  const ref = useRef<T>(null)
  const [width, setWidth] = useState(600)
  useEffect(() => {
    if (!ref.current) return
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width))
    observer.observe(ref.current)
    return () => observer.disconnect()
  }, [])
  return [ref, width] as const
}

function niceTicks(min: number, max: number, count = 4): number[] {
  const span = max - min || 1
  const step = 10 ** Math.floor(Math.log10(span / count))
  const nice = [1, 2, 2.5, 5, 10].map((m) => m * step).find((s) => span / s <= count) ?? step * 10
  const ticks = []
  for (let v = Math.ceil(min / nice) * nice; v <= max + 1e-9; v += nice) ticks.push(Number(v.toFixed(6)))
  return ticks
}

function timeLabel(t: number, spanMs: number): string {
  const d = new Date(t)
  return spanMs > 2 * 86_400_000
    ? d.toLocaleDateString('de-DE', { day: '2-digit', month: '2-digit' })
    : d.toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' })
}

/**
 * Liniendiagramm für eine Messgröße. `hover` ist zwischen allen Diagrammen
 * einer Seite geteilt, damit das Fadenkreuz überall auf derselben Zeit steht.
 */
export default function LineChart({
  title,
  unit,
  color,
  series,
  hover,
  onHover,
  target,
  domain,
  digits = 1,
  step = false,
}: {
  title: string
  unit: string
  color: string
  series: Series
  hover: number | null
  onHover: (index: number | null) => void
  /** Gestrichelte Ziellinie, z. B. Solltemperatur. */
  target?: number
  /** Feste Y-Achse, sonst aus den Daten. */
  domain?: [number, number]
  digits?: number
  /** Treppenstufen statt Linie (für Anteile wie „Heizung an“). */
  step?: boolean
}) {
  const [ref, width] = useWidth<HTMLDivElement>()
  const { times, values } = series
  const present = values.filter((v): v is number => v !== null)
  if (times.length === 0 || present.length === 0) {
    return (
      <div className="chart" ref={ref}>
        <div className="chart-head">
          <strong>{title}</strong>
          <span className="muted small">keine Daten</span>
        </div>
      </div>
    )
  }

  const [t0, t1] = [times[0], times[times.length - 1] || times[0] + 1]
  let [y0, y1] = domain ?? [Math.min(...present, target ?? Infinity), Math.max(...present, target ?? -Infinity)]
  if (!domain) {
    const pad = (y1 - y0) * 0.1 || 1
    y0 -= pad
    y1 += pad
  }
  const innerW = Math.max(width - PAD.left - PAD.right, 10)
  const innerH = HEIGHT - PAD.top - PAD.bottom
  const x = (t: number) => PAD.left + ((t - t0) / (t1 - t0 || 1)) * innerW
  const y = (v: number) => PAD.top + (1 - (v - y0) / (y1 - y0 || 1)) * innerH

  // Lücken (Sensor ausgefallen) unterbrechen die Linie.
  let path = ''
  let pen = false
  values.forEach((v, i) => {
    if (v === null) {
      pen = false
      return
    }
    if (!pen) path += `M${x(times[i])},${y(v)}`
    else if (step) path += `H${x(times[i])}V${y(v)}`
    else path += `L${x(times[i])},${y(v)}`
    pen = true
  })

  const index = hover !== null && hover < times.length ? hover : null
  const shown = index !== null ? values[index] : present[present.length - 1]
  const shownTime = index !== null ? times[index] : null

  function move(e: PointerEvent<SVGSVGElement>) {
    const rect = e.currentTarget.getBoundingClientRect()
    const t = t0 + ((e.clientX - rect.left - PAD.left) / innerW) * (t1 - t0)
    let best = 0
    for (let i = 1; i < times.length; i++) {
      if (Math.abs(times[i] - t) < Math.abs(times[best] - t)) best = i
    }
    onHover(best)
  }

  const spanMs = t1 - t0
  // Zeit-Ticks auf glatten Viertelstunden, Stunden oder Tagen (Ortszeit)
  const hourMs = 3_600_000
  const tickStep = [15 * 60_000, 30 * 60_000, hourMs, 3 * hourMs, 6 * hourMs, 12 * hourMs, 24 * hourMs, 7 * 24 * hourMs]
    .find((s) => spanMs / s <= Math.max(2, Math.floor(innerW / 90))) ?? 30 * 24 * hourMs
  const offset = new Date(t0).getTimezoneOffset() * 60_000
  const timeTicks: number[] = []
  for (let t = Math.ceil((t0 - offset) / tickStep) * tickStep + offset; t <= t1; t += tickStep) timeTicks.push(t)

  return (
    <div className="chart" ref={ref}>
      <div className="chart-head">
        <strong>{title}</strong>
        <span>
          <span className="value">
            {shown === null ? '–' : shown.toLocaleString('de-DE', { maximumFractionDigits: digits, minimumFractionDigits: digits })}{' '}
            {unit}
          </span>
          {shownTime !== null && (
            <span className="muted small"> · {new Date(shownTime).toLocaleString('de-DE', { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' })}</span>
          )}
        </span>
      </div>
      <svg
        height={HEIGHT}
        role="img"
        aria-label={`${title}, Verlauf`}
        onPointerMove={move}
        onPointerDown={move}
        onPointerLeave={() => onHover(null)}
      >
        <g className="axis">
          {niceTicks(y0, y1).map((v) => (
            <g key={v}>
              <line className="gridline" x1={PAD.left} x2={PAD.left + innerW} y1={y(v)} y2={y(v)} />
              <text x={PAD.left - 6} y={y(v)} dy="0.32em" textAnchor="end">
                {v.toLocaleString('de-DE')}
              </text>
            </g>
          ))}
          {timeTicks.map((t) => (
            <text key={t} x={x(t)} y={HEIGHT - 6} textAnchor="middle">
              {timeLabel(t, spanMs)}
            </text>
          ))}
        </g>
        {target !== undefined && <line className="target-line" x1={PAD.left} x2={PAD.left + innerW} y1={y(target)} y2={y(target)} />}
        <path d={path} fill="none" stroke={color} strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" />
        {index !== null && (
          <g>
            <line className="crosshair" x1={x(times[index])} x2={x(times[index])} y1={PAD.top} y2={PAD.top + innerH} />
            {values[index] !== null && (
              <circle cx={x(times[index])} cy={y(values[index])} r={4.5} fill={color} stroke="var(--surface)" strokeWidth={2} />
            )}
          </g>
        )}
      </svg>
    </div>
  )
}

import type { ApiError } from './generated/ApiError'

export class ApiFailure extends Error {
  status: number
  details: string[]

  constructor(status: number, message: string, details: string[] = []) {
    super(message)
    this.status = status
    this.details = details
  }
}

let onUnauthorized: () => void = () => {}

/** Wird aufgerufen, wenn die Session abgelaufen ist (HTTP 401). */
export function setUnauthorizedHandler(handler: () => void) {
  onUnauthorized = handler
}

export async function api<T>(path: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(path, {
    ...init,
    credentials: 'same-origin',
    headers: { 'Content-Type': 'application/json', ...init.headers },
  })
  if (!res.ok) {
    let error: ApiError = { message: `HTTP ${res.status}`, details: [] }
    try {
      error = (await res.json()) as ApiError
    } catch {
      // Antwort ohne JSON-Körper
    }
    if (res.status === 401 && !path.endsWith('/login')) onUnauthorized()
    throw new ApiFailure(res.status, error.message, error.details)
  }
  const type = res.headers.get('Content-Type') ?? ''
  return (type.includes('application/json') ? await res.json() : undefined) as T
}

export const post = <T>(path: string, body: unknown) =>
  api<T>(path, { method: 'POST', body: JSON.stringify(body) })

export const put = <T>(path: string, body: unknown) =>
  api<T>(path, { method: 'PUT', body: JSON.stringify(body) })

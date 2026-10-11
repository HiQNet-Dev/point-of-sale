import { afterEach, describe, expect, it, vi } from 'vitest'
import { configureApiAuth, requestJson } from './client'

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

afterEach(() => {
  configureApiAuth(null)
  vi.unstubAllGlobals()
})

describe('authenticated API requests', () => {
  it('adds a bearer header centrally and omits it when there is no session', async () => {
    const fetchMock = vi.fn().mockImplementation(async () => json({ ok: true }))
    vi.stubGlobal('fetch', fetchMock)
    configureApiAuth({
      getAccessToken: () => 'access-1',
      refreshAccessToken: async () => 'access-2',
    })

    await requestJson('/private')
    expect(new Headers(fetchMock.mock.calls[0]?.[1]?.headers).get('Authorization')).toBe('Bearer access-1')

    configureApiAuth({ getAccessToken: () => null, refreshAccessToken: async () => 'unused' })
    await requestJson('/public')
    expect(new Headers(fetchMock.mock.calls[1]?.[1]?.headers).has('Authorization')).toBe(false)
  })

  it('single-flights refresh across simultaneous 401s, retries each original request once', async () => {
    let accessToken = 'expired-access'
    let refreshCalls = 0
    const attempts = new Map<string, number>()
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(String(input), 'http://localhost')
      const attempt = (attempts.get(url.pathname) ?? 0) + 1
      attempts.set(url.pathname, attempt)
      const authorization = new Headers(init?.headers).get('Authorization')

      if (attempt === 1) {
        if (url.pathname.endsWith('/slow')) await new Promise((resolve) => setTimeout(resolve, 25))
        return json({ code: 'TOKEN_EXPIRED', message: 'Expired' }, 401)
      }
      return json({ url: url.pathname, authorization })
    })
    vi.stubGlobal('fetch', fetchMock)
    configureApiAuth({
      getAccessToken: () => accessToken,
      refreshAccessToken: async () => {
        refreshCalls += 1
        await new Promise((resolve) => setTimeout(resolve, 1))
        accessToken = 'fresh-access'
        return accessToken
      },
    })

    const results = await Promise.all([
      requestJson<{ authorization: string }>('/one'),
      requestJson<{ authorization: string }>('/two'),
      requestJson<{ authorization: string }>('/slow'),
    ])

    expect(refreshCalls).toBe(1)
    expect(results).toHaveLength(3)
    expect(results.every((result) => result.authorization === 'Bearer fresh-access')).toBe(true)
    expect([...attempts.values()]).toEqual([2, 2, 2])
  })

  it('does not retry an original request again after the refreshed request also gets 401', async () => {
    const fetchMock = vi.fn().mockResolvedValue(json({ code: 'TOKEN_INVALID', message: 'Invalid' }, 401))
    const refresh = vi.fn().mockResolvedValue('fresh-access')
    vi.stubGlobal('fetch', fetchMock)
    configureApiAuth({ getAccessToken: () => 'old-access', refreshAccessToken: refresh })

    await expect(requestJson('/private')).rejects.toMatchObject({ status: 401, code: 'TOKEN_INVALID' })
    expect(refresh).toHaveBeenCalledTimes(1)
    expect(fetchMock).toHaveBeenCalledTimes(2)
  })

  it('never retries a request from a prior session using a newly logged-in user token', async () => {
    let accessToken = 'user-a-access'
    let sessionId = 1
    let finishRequest: ((response: Response) => void) | undefined
    let initialAuthorization = ''
    const fetchMock = vi.fn((_input: RequestInfo | URL, init?: RequestInit) => {
      initialAuthorization = new Headers(init?.headers).get('Authorization') ?? ''
      return new Promise<Response>((resolve) => { finishRequest = resolve })
    })
    const refresh = vi.fn().mockResolvedValue('unused')
    vi.stubGlobal('fetch', fetchMock)
    configureApiAuth({
      getAccessToken: () => accessToken,
      getSessionId: () => sessionId,
      refreshAccessToken: refresh,
    })

    const request = requestJson('/private')
    accessToken = 'user-b-access'
    sessionId = 2
    finishRequest?.(json({ code: 'TOKEN_INVALID', message: 'Invalid' }, 401))

    await expect(request).rejects.toMatchObject({ status: 401, code: 'TOKEN_INVALID' })
    expect(refresh).not.toHaveBeenCalled()
    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(initialAuthorization).toBe('Bearer user-a-access')
  })

  it('does not make a new user wait for a refresh started by the previous session', async () => {
    let accessToken = 'user-a-access'
    let sessionId = 1
    let startUserARefresh: (() => void) | undefined
    let rejectUserARefresh: ((error: Error) => void) | undefined
    const userARefreshStarted = new Promise<void>((resolve) => { startUserARefresh = resolve })
    const userARefresh = new Promise<string>((_resolve, reject) => { rejectUserARefresh = reject })
    const attempts = new Map<string, number>()
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(String(input), 'http://localhost')
      const attempt = (attempts.get(url.pathname) ?? 0) + 1
      attempts.set(url.pathname, attempt)
      if (attempt === 1) return json({ code: 'TOKEN_EXPIRED', message: 'Expired' }, 401)
      return json({ authorization: new Headers(init?.headers).get('Authorization') })
    })
    vi.stubGlobal('fetch', fetchMock)
    configureApiAuth({
      getAccessToken: () => accessToken,
      getSessionId: () => sessionId,
      refreshAccessToken: async () => {
        if (sessionId === 1) {
          startUserARefresh?.()
          return userARefresh
        }
        accessToken = 'user-b-fresh-access'
        return accessToken
      },
    })

    const userARequest = requestJson('/user-a')
    await userARefreshStarted
    accessToken = 'user-b-access'
    sessionId = 2

    const userBResponse = await requestJson<{ authorization: string }>('/user-b')
    expect(userBResponse.authorization).toBe('Bearer user-b-fresh-access')

    rejectUserARefresh?.(new Error('old session refresh ended'))
    await expect(userARequest).rejects.toThrow('old session refresh ended')
    expect(attempts.get('/api/v1/user-a')).toBe(1)
    expect(attempts.get('/api/v1/user-b')).toBe(2)
  })

  it('does not attempt token recovery for public requests', async () => {
    const fetchMock = vi.fn().mockResolvedValue(json({ code: 'INVALID_CREDENTIALS', message: 'Invalid' }, 401))
    const refresh = vi.fn().mockResolvedValue('unused')
    vi.stubGlobal('fetch', fetchMock)
    configureApiAuth({ getAccessToken: () => 'access', refreshAccessToken: refresh })

    await expect(requestJson('/auth/login', { method: 'POST' }, { authenticated: false })).rejects.toMatchObject({
      status: 401,
      code: 'INVALID_CREDENTIALS',
    })
    expect(refresh).not.toHaveBeenCalled()
    expect(fetchMock).toHaveBeenCalledTimes(1)
  })
})

// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { Navigate, createMemoryRouter, RouterProvider } from 'react-router-dom'
import { requestJson, configureApiAuth } from '../../../shared/api/client'
import type { AuthMe, TokenResponse } from '../api/authApi'
import { LoginPage } from '../pages/LoginPage'
import { SessionHomePage } from '../pages/SessionHomePage'
import { LoginRoute } from '../components/LoginRoute'
import { ProtectedRoute } from '../components/ProtectedRoute'
import { AuthSessionProvider } from './AuthSessionProvider'
import { authTokenStore } from './authTokenStore'

function tokenResponse(index = 1): TokenResponse {
  return {
    accessToken: `access-${index}`,
    tokenType: 'Bearer',
    expiresIn: 600,
    refreshToken: `refresh-${index}`,
    refreshExpiresAt: '2026-11-07T00:00:00Z',
  }
}

function meResponse(email: string): AuthMe {
  return {
    userId: `user-${email}`,
    organizationId: 'org-1',
    email,
    status: 'ACTIVE',
    organization: { currency: 'MXN', locale: 'es-MX', timezone: 'America/Mexico_City' },
  }
}

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

function RequestProbe() {
  return (
    <button type="button" onClick={() => void Promise.all([
      requestJson('/probe-one'),
      requestJson('/probe-two'),
      requestJson('/probe-three'),
    ]).catch(() => undefined)}>
      Test protected requests
    </button>
  )
}

function renderAuthApp(initialEntry = '/') {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const protectedPage = (
    <ProtectedRoute>
      <>
        <SessionHomePage />
        <RequestProbe />
      </>
    </ProtectedRoute>
  )
  const router = createMemoryRouter([
    { path: '/login', element: <LoginRoute><LoginPage /></LoginRoute> },
    { path: '/', element: protectedPage },
    { path: '/workspace', element: protectedPage },
    { path: '*', element: <Navigate to="/" replace /> },
  ], { initialEntries: [initialEntry] })

  const view = render(
    <QueryClientProvider client={queryClient}>
      <AuthSessionProvider>
        <RouterProvider router={router} />
      </AuthSessionProvider>
    </QueryClientProvider>,
  )
  return { ...view, queryClient, router }
}

async function enterCredentials(organizationCode = 'cafe', email = 'operator@example.com') {
  fireEvent.change(screen.getByLabelText(/Organization code/), { target: { value: organizationCode } })
  fireEvent.change(screen.getByLabelText(/Email/), { target: { value: email } })
  fireEvent.change(screen.getByLabelText(/Password/), { target: { value: 'Correct-horse-battery-1' } })
  fireEvent.click(screen.getByRole('button', { name: 'Sign in' }))
}

afterEach(() => {
  cleanup()
  authTokenStore.clear()
  configureApiAuth(null)
  vi.unstubAllGlobals()
})

describe('frontend authentication session', () => {
  it('bootstraps neutrally, logs in, loads /me, preserves destination, and redirects authenticated /login', async () => {
    let currentEmail = ''
    let logins = 0
    let logoutBody: { refreshToken: string } | null = null
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input)
      if (url.endsWith('/auth/login')) {
        logins += 1
        currentEmail = JSON.parse(String(init?.body)).email
        return json(tokenResponse(logins))
      }
      if (url.endsWith('/auth/me')) {
        expect(new Headers(init?.headers).get('Authorization')).toBe(`Bearer access-${logins}`)
        return json(meResponse(currentEmail))
      }
      if (url.endsWith('/auth/logout')) {
        logoutBody = JSON.parse(String(init?.body))
        return new Response(null, { status: 204 })
      }
      return new Response(null, { status: 204 })
    })
    vi.stubGlobal('fetch', fetchMock)

    const { router } = renderAuthApp('/workspace?tab=context')
    expect(await screen.findByRole('heading', { name: 'Sign in' })).toBeTruthy()
    expect(router.state.location.state).toMatchObject({ from: { pathname: '/workspace', search: '?tab=context' } })
    expect(fetchMock).not.toHaveBeenCalled()

    await enterCredentials()
    expect(await screen.findByRole('heading', { name: 'Session active' })).toBeTruthy()
    expect(screen.getByText('operator@example.com')).toBeTruthy()
    expect(screen.getByText('America/Mexico_City')).toBeTruthy()
    expect(router.state.location.pathname).toBe('/workspace')
    expect(router.state.location.search).toBe('?tab=context')
    expect(logins).toBe(1)

    await router.navigate('/login')
    await waitFor(() => expect(router.state.location.pathname).toBe('/'))
    expect(screen.getByRole('heading', { name: 'Session active' })).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'Sign out' }))
    expect(await screen.findByRole('heading', { name: 'Sign in' })).toBeTruthy()
    expect(authTokenStore.getAccessToken()).toBeNull()
    expect(logoutBody).toEqual({ refreshToken: 'refresh-1' })
  })

  it('keeps protected content hidden until /auth/me finishes bootstrapping', async () => {
    let finishMe: ((response: Response) => void) | undefined
    const pendingMe = new Promise<Response>((resolve) => { finishMe = resolve })
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      if (String(input).endsWith('/auth/login')) return json(tokenResponse())
      if (String(input).endsWith('/auth/me')) return pendingMe
      return new Response(null, { status: 204 })
    })
    vi.stubGlobal('fetch', fetchMock)

    renderAuthApp('/login')
    await screen.findByRole('heading', { name: 'Sign in' })
    await enterCredentials()
    expect(await screen.findByText('Checking your session…')).toBeTruthy()
    expect(screen.queryByRole('heading', { name: 'Session active' })).toBeNull()

    finishMe?.(json(meResponse('operator@example.com')))
    expect(await screen.findByRole('heading', { name: 'Session active' })).toBeTruthy()
  })

  it('shows generic credential errors and prevents duplicate pending submissions', async () => {
    let finishLogin: ((response: Response) => void) | undefined
    const pendingLogin = new Promise<Response>((resolve) => { finishLogin = resolve })
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      if (String(input).endsWith('/auth/login')) return pendingLogin
      return json(meResponse('operator@example.com'))
    })
    vi.stubGlobal('fetch', fetchMock)

    renderAuthApp('/login')
    await screen.findByRole('heading', { name: 'Sign in' })
    await enterCredentials()

    const submit = screen.getByRole('button', { name: 'Signing in…' })
    expect(submit.hasAttribute('disabled')).toBe(true)
    expect((screen.getByLabelText(/Password/) as HTMLInputElement).value).toBe('')
    fireEvent.click(submit)
    expect(fetchMock.mock.calls.filter(([input]) => String(input).endsWith('/auth/login'))).toHaveLength(1)

    finishLogin?.(json({ code: 'INVALID_CREDENTIALS', message: 'Invalid credentials' }, 401))
    expect(await screen.findByRole('alert')).toBeTruthy()
    expect(screen.getByText('The organization code, email, or password is incorrect.')).toBeTruthy()
  })

  it('validates an email with the native accessible form constraint before sending login', async () => {
    const fetchMock = vi.fn()
    vi.stubGlobal('fetch', fetchMock)

    renderAuthApp('/login')
    await screen.findByRole('heading', { name: 'Sign in' })
    fireEvent.change(screen.getByLabelText(/Organization code/), { target: { value: 'cafe' } })
    fireEvent.change(screen.getByLabelText(/Email/), { target: { value: 'not-an-email' } })
    fireEvent.change(screen.getByLabelText(/Password/), { target: { value: 'password' } })
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }))

    expect((screen.getByLabelText(/Email/) as HTMLInputElement).validity.typeMismatch).toBe(true)
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('shows a safe network error without exposing fetch details', async () => {
    const fetchMock = vi.fn().mockRejectedValue(new TypeError('private network diagnostic'))
    vi.stubGlobal('fetch', fetchMock)

    renderAuthApp('/login')
    await screen.findByRole('heading', { name: 'Sign in' })
    await enterCredentials()

    expect(await screen.findByRole('alert')).toBeTruthy()
    expect(screen.getByText('Unable to connect to the server. Check your connection and try again.')).toBeTruthy()
    expect(screen.queryByText(/private network diagnostic/)).toBeNull()
  })

  it('rotates refresh once for concurrent requests and retries each original request once', async () => {
    let currentTokens = tokenResponse()
    let refreshCalls = 0
    const attempts = new Map<string, number>()
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(String(input), 'http://localhost')
      if (url.pathname.endsWith('/auth/login')) return json(currentTokens)
      if (url.pathname.endsWith('/auth/me')) return json(meResponse('operator@example.com'))
      if (url.pathname.endsWith('/auth/refresh')) {
        refreshCalls += 1
        expect(JSON.parse(String(init?.body))).toEqual({ refreshToken: 'refresh-1' })
        expect(new Headers(init?.headers).has('Authorization')).toBe(false)
        await new Promise((resolve) => setTimeout(resolve, 10))
        currentTokens = tokenResponse(2)
        return json(currentTokens)
      }
      const attempt = (attempts.get(url.pathname) ?? 0) + 1
      attempts.set(url.pathname, attempt)
      const authorization = new Headers(init?.headers).get('Authorization')
      if (attempt === 1) return json({ code: 'TOKEN_EXPIRED', message: 'Expired' }, 401)
      return json({ authorization })
    })
    vi.stubGlobal('fetch', fetchMock)

    renderAuthApp('/login')
    await screen.findByRole('heading', { name: 'Sign in' })
    await enterCredentials()
    await screen.findByRole('heading', { name: 'Session active' })

    fireEvent.click(screen.getByRole('button', { name: 'Test protected requests' }))
    await waitFor(() => expect([...attempts.values()]).toEqual([2, 2, 2]))
    expect(refreshCalls).toBe(1)
    expect(authTokenStore.getAccessToken()).toBe('access-2')
  })

  it('clears credentials and authenticated query data when refresh fails', async () => {
    let refreshCalls = 0
    let probeCalls = 0
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input)
      if (url.endsWith('/auth/login')) return json(tokenResponse())
      if (url.endsWith('/auth/me')) return json(meResponse('operator@example.com'))
      if (url.endsWith('/auth/refresh')) {
        refreshCalls += 1
        return json({ code: 'INVALID_REFRESH_TOKEN', message: 'Invalid refresh token' }, 401)
      }
      if (url.includes('/probe-')) {
        probeCalls += 1
        return json({ code: 'TOKEN_EXPIRED', message: 'Expired' }, 401)
      }
      return new Response(null, { status: 204 })
    })
    vi.stubGlobal('fetch', fetchMock)

    const { queryClient } = renderAuthApp('/login')
    await screen.findByRole('heading', { name: 'Sign in' })
    await enterCredentials()
    await screen.findByRole('heading', { name: 'Session active' })
    queryClient.setQueryData(['private', 'user-a'], { privateValue: 'cached' })

    fireEvent.click(screen.getByRole('button', { name: 'Test protected requests' }))
    await screen.findByRole('heading', { name: 'Sign in' })

    expect(refreshCalls).toBe(1)
    expect(probeCalls).toBe(3)
    expect(authTokenStore.getAccessToken()).toBeNull()
    expect(queryClient.getQueryData(['private', 'user-a'])).toBeUndefined()
    expect(screen.getByText('Your session ended. Please sign in again.')).toBeTruthy()
  })

  it('clears query cache on logout even when remote revocation fails, then isolates the next user', async () => {
    let currentEmail = ''
    let loginCount = 0
    let logoutBody: { refreshToken: string } | null = null
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input)
      if (url.endsWith('/auth/login')) {
        loginCount += 1
        currentEmail = JSON.parse(String(init?.body)).email
        return json(tokenResponse(loginCount))
      }
      if (url.endsWith('/auth/me')) return json(meResponse(currentEmail))
      if (url.endsWith('/auth/logout')) {
        logoutBody = JSON.parse(String(init?.body))
        return json({ code: 'INTERNAL_ERROR', message: 'Unavailable' }, 503)
      }
      return new Response(null, { status: 204 })
    })
    vi.stubGlobal('fetch', fetchMock)

    const { queryClient } = renderAuthApp('/login')
    await screen.findByRole('heading', { name: 'Sign in' })
    await enterCredentials('cafe', 'first@example.com')
    await screen.findByRole('heading', { name: 'Session active' })
    queryClient.setQueryData(['private', 'records'], ['first user data'])

    fireEvent.click(screen.getByRole('button', { name: 'Sign out' }))
    await screen.findByRole('heading', { name: 'Sign in' })
    expect(queryClient.getQueryData(['private', 'records'])).toBeUndefined()
    expect(await screen.findByText('You are signed out locally, but server revocation could not be confirmed.')).toBeTruthy()
    expect(logoutBody).toEqual({ refreshToken: 'refresh-1' })

    await enterCredentials('cafe', 'second@example.com')
    await screen.findByRole('heading', { name: 'Session active' })
    expect(screen.getByText('second@example.com')).toBeTruthy()
    expect(queryClient.getQueryData(['private', 'records'])).toBeUndefined()
    expect(authTokenStore.getRefreshToken()).toBe('refresh-2')
  })

  it('does not interpret or expose the access token in the rendered session UI', async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      if (String(input).endsWith('/auth/login')) return json(tokenResponse())
      if (String(input).endsWith('/auth/me')) return json(meResponse('operator@example.com'))
      return new Response(null, { status: 204 })
    })
    vi.stubGlobal('fetch', fetchMock)

    renderAuthApp('/login')
    await screen.findByRole('heading', { name: 'Sign in' })
    await enterCredentials()
    await screen.findByRole('heading', { name: 'Session active' })

    expect(screen.queryByText(/access-1|refresh-1/)).toBeNull()
    expect(screen.queryByText(/administrator|cashier|manager|permission/i)).toBeNull()
  })
})

import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { ApiRequestError, configureApiAuth } from '../../../shared/api/client'
import type { ApiAuthAdapter } from '../../../shared/api/client'
import { getMe, login as loginRequest, logout as logoutRequest, refresh } from '../api/authApi'
import type { AuthMe, LoginRequest } from '../api/authApi'
import { authKeys } from './authKeys'
import { authTokenStore } from './authTokenStore'
import { AuthSessionContext } from './authSessionContext'
import type { AuthSessionStatus } from './authSessionContext'

class AuthSessionReplacedError extends Error {
  constructor() {
    super('The active session changed while refresh was in progress.')
    this.name = 'AuthSessionReplacedError'
  }
}

export function AuthSessionProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient()
  const [status, setStatus] = useState<AuthSessionStatus>(() =>
    authTokenStore.getAccessToken() ? 'BOOTSTRAPPING' : 'UNAUTHENTICATED',
  )
  const [logoutNotice, setLogoutNotice] = useState<string | null>(null)

  const onRefreshFailure = useCallback(() => {
    queryClient.clear()
    setStatus('UNAUTHENTICATED')
    setLogoutNotice('Your session ended. Please sign in again.')
  }, [queryClient])

  const authAdapter = useMemo<ApiAuthAdapter>(
    () => ({
      getAccessToken: () => authTokenStore.getAccessToken(),
      getSessionId: () => authTokenStore.getSessionId(),
      refreshAccessToken: async () => {
        const snapshot = authTokenStore.snapshot()
        if (!snapshot.tokens) {
          const error = new ApiRequestError(401, 'INVALID_REFRESH_TOKEN', 'No active refresh session.')
          onRefreshFailure()
          throw error
        }

        try {
          const next = await refresh(snapshot.tokens.refreshToken)
          if (!authTokenStore.rotate(next, snapshot.revision)) throw new AuthSessionReplacedError()
          return next.accessToken
        } catch (error) {
          if (authTokenStore.isCurrent(snapshot.revision)) {
            authTokenStore.clear()
            onRefreshFailure()
          }
          throw error
        }
      },
      onAuthorizationFailure: () => {
        authTokenStore.clear()
        onRefreshFailure()
      },
    }),
    [onRefreshFailure],
  )

  useEffect(() => {
    configureApiAuth(authAdapter)
    return () => configureApiAuth(null)
  }, [authAdapter])

  const fetchMeForCurrentSession = useCallback(async () => {
    const sessionId = authTokenStore.getSessionId()
    try {
      return await getMe()
    } catch (error) {
      if (sessionId !== null && authTokenStore.getSessionId() === sessionId) {
        authTokenStore.clear()
        queryClient.clear()
        setStatus('UNAUTHENTICATED')
      }
      throw error
    }
  }, [queryClient])

  const meQuery = useQuery<AuthMe, Error>({
    queryKey: authKeys.me,
    queryFn: fetchMeForCurrentSession,
    enabled: status === 'BOOTSTRAPPING' && Boolean(authTokenStore.getAccessToken()),
    staleTime: Infinity,
    retry: false,
  })

  const login = useCallback(
    async (request: LoginRequest) => {
      queryClient.clear()
      authTokenStore.clear()
      setLogoutNotice(null)

      const tokens = await loginRequest(request)
      authTokenStore.set(tokens)
      const loginSessionId = authTokenStore.getSessionId()
      setStatus('BOOTSTRAPPING')

      try {
        const me = await queryClient.fetchQuery({
          queryKey: authKeys.me,
          queryFn: fetchMeForCurrentSession,
          staleTime: 0,
          retry: false,
        })
        setStatus('AUTHENTICATED')
        return me
      } catch (error) {
        if (authTokenStore.getSessionId() === loginSessionId) {
          authTokenStore.clear()
          queryClient.clear()
          setStatus('UNAUTHENTICATED')
        }
        throw error
      }
    },
    [fetchMeForCurrentSession, queryClient],
  )

  const logout = useCallback(async () => {
    const refreshToken = authTokenStore.getRefreshToken()
    authTokenStore.clear()
    const logoutRevision = authTokenStore.snapshot().revision
    queryClient.clear()
    setStatus('UNAUTHENTICATED')
    setLogoutNotice(null)

    if (!refreshToken) return true
    try {
      await logoutRequest(refreshToken)
      return true
    } catch {
      if (authTokenStore.isCurrent(logoutRevision)) {
        setLogoutNotice('You are signed out locally, but server revocation could not be confirmed.')
      }
      return false
    }
  }, [queryClient])

  const visibleStatus = status === 'BOOTSTRAPPING' && meQuery.isSuccess ? 'AUTHENTICATED' : status
  const value = useMemo(
    () => ({
      status: visibleStatus,
      me: visibleStatus === 'AUTHENTICATED' ? meQuery.data ?? null : null,
      logoutNotice,
      login,
      logout,
    }),
    [login, logout, logoutNotice, meQuery.data, visibleStatus],
  )

  return <AuthSessionContext.Provider value={value}>{children}</AuthSessionContext.Provider>
}

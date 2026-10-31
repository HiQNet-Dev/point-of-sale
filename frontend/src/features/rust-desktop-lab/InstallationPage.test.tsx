// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { InstallationPage } from './InstallationPage'

afterEach(() => { cleanup() })

describe('Instalación persistente no equivale a identidad autorizada', () => {
  for (const [state, label] of [['FIRST_INSTALL', 'Primera instalación'], ['REOPENED', 'Instalación reabierta']]) {
    it(`mantiene identidad bloqueada en ${state}`, () => {
      render(<InstallationPage status={{ state, schemaVersion: 1, appliedMigrations: 1 }} />)
      expect(screen.getByText(label)).toBeTruthy()
      expect(screen.getByRole('status').textContent).toContain('Identidad no habilitada')
      expect(screen.getByRole('status').textContent).toContain('no acredita un administrador ni una sesión válida')
      expect(screen.queryByRole('button', { name: /login|iniciar sesión|aprovisionar|provisioning/i })).toBeNull()
      expect(screen.queryByRole('textbox', { name: /password|contraseña|token/i })).toBeNull()
    })
  }
})

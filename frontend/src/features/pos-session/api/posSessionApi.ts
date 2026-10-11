import { requestJson } from '../../../shared/api/client'

export interface PageResponse<T> {
  items: T[]
  page: number
  size: number
  totalElements: number
  totalPages: number
}

export interface Branch {
  id: string
  organizationId: string
  name: string
  code: string
  timezone: string
  status: 'ACTIVE' | 'INACTIVE'
}

export interface CashRegister {
  id: string
  organizationId: string
  branchId: string
  code: string
  name: string
  status: 'ACTIVE' | 'INACTIVE'
}

export interface CashShift {
  id: string
  organizationId: string
  branchId: string
  cashRegisterId: string
  status: 'OPEN' | 'CLOSED'
  openingAmount: number
  closingDeclaredAmount?: number
  openedAt: string
  closedAt?: string
  version: number
}

export interface ListPage {
  page: number
  size: number
  q?: string
}

const PAGE_SIZE = 20

function pagingParams({ page, size, q }: ListPage): string {
  const params = new URLSearchParams({ page: String(page), size: String(size) })
  if (q?.trim()) params.set('q', q.trim())
  return params.toString()
}

function segment(value: string): string {
  return encodeURIComponent(value)
}

export function listMyBranches(query: ListPage): Promise<PageResponse<Branch>> {
  return requestJson(`/auth/me/branches?${pagingParams(query)}`)
}

export function listCashRegisters(
  organizationId: string,
  branchId: string,
  query: ListPage,
): Promise<PageResponse<CashRegister>> {
  return requestJson(
    `/organizations/${segment(organizationId)}/branches/${segment(branchId)}/cash-registers?${pagingParams(query)}`,
  )
}

export function listOpenCashShifts(
  organizationId: string,
  branchId: string,
  cashRegisterId: string,
): Promise<PageResponse<CashShift>> {
  const params = new URLSearchParams({ page: '0', size: String(PAGE_SIZE), status: 'OPEN' })
  return requestJson(
    `/organizations/${segment(organizationId)}/branches/${segment(branchId)}`
      + `/cash-registers/${segment(cashRegisterId)}/cash-shifts?${params.toString()}`,
  )
}

export function openCashShift(
  organizationId: string,
  branchId: string,
  cashRegisterId: string,
  openingAmount: string,
): Promise<CashShift> {
  return requestJson(
    `/organizations/${segment(organizationId)}/branches/${segment(branchId)}`
      + `/cash-registers/${segment(cashRegisterId)}/cash-shifts`,
    { method: 'POST', body: JSON.stringify({ openingAmount }) },
  )
}

export function closeCashShift(
  organizationId: string,
  branchId: string,
  cashRegisterId: string,
  cashShiftId: string,
  expectedVersion: number,
  declaredCashAmount: string,
): Promise<CashShift> {
  return requestJson(
    `/organizations/${segment(organizationId)}/branches/${segment(branchId)}`
      + `/cash-registers/${segment(cashRegisterId)}/cash-shifts/${segment(cashShiftId)}/close`,
    {
      method: 'POST',
      body: JSON.stringify({ expectedVersion, declaredCashAmount }),
    },
  )
}

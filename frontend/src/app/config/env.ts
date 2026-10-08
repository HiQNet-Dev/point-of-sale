/**
 * Environment configuration.
 * Never hardcode backend URLs in business code; use this module.
 * The base URL comes from VITE_API_BASE_URL and defaults to the same-origin
 * API prefix (suitable behind a reverse proxy or the Vite dev proxy).
 */
export const API_BASE_URL: string = import.meta.env.VITE_API_BASE_URL ?? '/api/v1'
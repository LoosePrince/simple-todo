import { invoke } from '@tauri-apps/api/core'
import { defineStore } from 'pinia'
import { defaultSyncConfig, useSettingsStore } from './settings'

export type SyncPhase = 'idle' | 'syncing' | 'success' | 'error' | 'conflict'

export type SyncNowResult = {
  uploaded_count: number
  downloaded_count: number
  skipped_count: number
  conflict_count: number
  errors: string[]
  last_sync_at: number
}

export type SyncConflictRecord = {
  id: string
  path: string
  local_hash?: string | null
  remote_hash?: string | null
  base_hash?: string | null
  local_modified_at?: number | null
  remote_modified_at?: number | null
  reason: string
  suggested_action: string
  status: string
  created_at: number
  resolved_at?: number | null
  resolved_action?: string | null
}

function syncPayload(password: string | null = null) {
  const settingsStore = useSettingsStore()
  return {
    data_path: settingsStore.config.data_path,
    webdav_url: settingsStore.config.webdav_url || '',
    webdav_username: settingsStore.config.webdav_username || '',
    webdav_remote_dir: settingsStore.config.webdav_remote_dir || defaultSyncConfig.webdav_remote_dir,
    password,
    conflict_default_action: settingsStore.config.sync_conflict_default_action || 'ask',
  }
}

export const useSyncStore = defineStore('sync', {
  state: () => ({
    phase: 'idle' as SyncPhase,
    lastResult: null as SyncNowResult | null,
    lastError: '',
    conflicts: [] as SyncConflictRecord[],
    syncing: false,
    pendingAfterCurrent: false,
    changeTimer: null as ReturnType<typeof setTimeout> | null,
    intervalTimer: null as ReturnType<typeof setInterval> | null,
  }),
  getters: {
    conflictCount: (state) => state.conflicts.length,
  },
  actions: {
    async loadConflicts() {
      const settingsStore = useSettingsStore()
      if (!settingsStore.config.data_path) return
      this.conflicts = await invoke<SyncConflictRecord[]>('list_sync_conflicts', {
        dataPath: settingsStore.config.data_path,
      })
      if (this.conflicts.length > 0 && this.phase !== 'syncing') this.phase = 'conflict'
    },
    async syncNow(password: string | null = null) {
      const settingsStore = useSettingsStore()
      if (!settingsStore.config.sync_enabled || !settingsStore.config.data_path) return null
      if (this.syncing) {
        this.pendingAfterCurrent = true
        return this.lastResult
      }
      this.syncing = true
      this.phase = 'syncing'
      this.lastError = ''
      try {
        const result = await invoke<SyncNowResult>('sync_now', {
          input: syncPayload(password),
        })
        this.lastResult = result
        await this.loadConflicts()
        this.phase = this.conflicts.length > 0 || result.conflict_count > 0 ? 'conflict' : 'success'
        if (result.errors.length > 0) {
          this.lastError = result.errors.join('\n')
          this.phase = 'error'
        }
        return result
      } catch (e) {
        this.lastError = e instanceof Error ? e.message : String(e)
        this.phase = 'error'
        throw e
      } finally {
        this.syncing = false
        if (this.pendingAfterCurrent) {
          this.pendingAfterCurrent = false
          this.scheduleChangeSync()
        }
      }
    },
    scheduleChangeSync() {
      const settingsStore = useSettingsStore()
      if (!settingsStore.config.sync_enabled || !settingsStore.config.sync_on_change) return
      if (this.changeTimer) clearTimeout(this.changeTimer)
      this.changeTimer = setTimeout(() => {
        this.changeTimer = null
        this.syncNow().catch(() => {})
      }, 2500)
    },
    startAutoSync() {
      const settingsStore = useSettingsStore()
      this.stopAutoSync()
      if (!settingsStore.config.sync_enabled) return
      if (settingsStore.config.sync_on_startup) {
        this.syncNow().catch(() => {})
      } else {
        this.loadConflicts().catch(() => {})
      }
      const seconds = Math.max(60, Number(settingsStore.config.sync_interval_seconds || 300))
      this.intervalTimer = setInterval(() => {
        this.syncNow().catch(() => {})
      }, seconds * 1000)
    },
    stopAutoSync() {
      if (this.changeTimer) clearTimeout(this.changeTimer)
      if (this.intervalTimer) clearInterval(this.intervalTimer)
      this.changeTimer = null
      this.intervalTimer = null
    },
    async resolveConflict(conflictId: string, action: 'keep_local' | 'use_remote' | 'duplicate_remote' | 'mark_resolved' | 'later', password: string | null = null) {
      if (action === 'later') return
      this.syncing = true
      this.phase = 'syncing'
      try {
        const payload = syncPayload(password)
        const result = await invoke<SyncNowResult>('resolve_sync_conflict', {
          input: {
            data_path: payload.data_path,
            webdav_url: payload.webdav_url,
            webdav_username: payload.webdav_username,
            webdav_remote_dir: payload.webdav_remote_dir,
            password: payload.password,
            conflict_id: conflictId,
            action,
          },
        })
        this.lastResult = result
        await this.loadConflicts()
        this.phase = this.conflicts.length > 0 ? 'conflict' : 'success'
        return result
      } catch (e) {
        this.lastError = e instanceof Error ? e.message : String(e)
        this.phase = 'error'
        throw e
      } finally {
        this.syncing = false
      }
    },
  },
})
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import i18n from '../i18n'

function getStartupTheme() {
  const theme = new URLSearchParams(window.location.search).get('theme')
  if (theme === 'dark' || theme === 'light') return theme
  return document.documentElement.classList.contains('dark') ? 'dark' : 'light'
}

export interface AppConfig {
  data_path: string
  language: string
  theme: string
  font_family: string
  font_size: number
  text_color_light: string
  text_color_dark: string
  launch_at_login?: boolean
  quick_record_shortcut?: string
  hide_quick_record_shortcut?: string
  sync_enabled?: boolean
  sync_provider?: 'webdav'
  webdav_url?: string
  webdav_username?: string
  webdav_remote_dir?: string
  sync_interval_seconds?: number
  sync_on_startup?: boolean
  sync_on_change?: boolean
  sync_conflict_default_action?: 'ask' | 'keep_local' | 'use_remote' | 'duplicate_remote'
}

export const syncConflictActions = [
  'ask',
  'keep_local',
  'use_remote',
  'duplicate_remote'
] as const

export const defaultSyncConfig = {
  sync_enabled: false,
  sync_provider: 'webdav' as const,
  webdav_url: '',
  webdav_username: '',
  webdav_remote_dir: '/simple-todo',
  sync_interval_seconds: 300,
  sync_on_startup: true,
  sync_on_change: true,
  sync_conflict_default_action: 'ask' as const,
}

export const useSettingsStore = defineStore('settings', {
  state: () => ({
    config: {
      data_path: '',
      language: 'zh-CN',
      theme: getStartupTheme(),
      font_family: 'Arial',
      font_size: 14,
      text_color_light: '#333333',
      text_color_dark: '#e5e5e5',
      launch_at_login: false,
      quick_record_shortcut: 'Ctrl+Shift+N',
      hide_quick_record_shortcut: 'Ctrl+Shift+H',
      ...defaultSyncConfig,
    } as AppConfig,
  }),
  getters: {
    globalStyles: (state) => ({
      '--app-font-family': state.config.font_family,
      '--app-font-size': `${state.config.font_size}px`,
      '--app-text-color': state.config.theme === 'dark' ? state.config.text_color_dark : state.config.text_color_light,
      '--app-bg-color': state.config.theme === 'dark' ? '#1a1a1a' : '#ffffff',
    }),
  },
  actions: {
    async loadConfig() {
      this.config = { ...defaultSyncConfig, ...(await invoke('get_app_config')) }
      if (this.config.launch_at_login == null) this.config.launch_at_login = false
      if (!this.config.quick_record_shortcut) this.config.quick_record_shortcut = 'Ctrl+Shift+N'
      if (!this.config.hide_quick_record_shortcut) this.config.hide_quick_record_shortcut = 'Ctrl+Shift+H'
      if (!this.config.sync_provider) this.config.sync_provider = 'webdav'
      if (!this.config.webdav_remote_dir) this.config.webdav_remote_dir = '/simple-todo'
      if (!this.config.sync_interval_seconds) this.config.sync_interval_seconds = 300
      if (!this.config.sync_conflict_default_action) this.config.sync_conflict_default_action = 'ask'
      this.applyI18n()
      this.applyTheme()
      await this.syncAutostart()
    },
    async syncAutostart() {
      try {
        const { enable, disable, isEnabled } = await import('@tauri-apps/plugin-autostart')
        const enabled = await isEnabled()
        if (this.config.launch_at_login && !enabled) await enable()
        else if (!this.config.launch_at_login && enabled) await disable()
      } catch (_) {}
    },
    async saveConfig() {
      await invoke('save_app_config', { config: this.config })
      this.applyI18n()
      this.applyTheme()
    },
    async updateConfig(newConfig: Partial<AppConfig>) {
      Object.assign(this.config, newConfig)
      await this.saveConfig()
    },
    applyI18n() {
      // @ts-ignore
      i18n.global.locale.value = this.config.language
    },
    applyTheme() {
      if (this.config.theme === 'dark') {
        document.documentElement.classList.add('dark')
      } else {
        document.documentElement.classList.remove('dark')
      }
    },
    async applySettings() {
      try {
        await this.loadConfig()
      } catch (_) {}
    }
  }
})

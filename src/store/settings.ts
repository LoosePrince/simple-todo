import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import i18n from '../i18n'

export interface AppConfig {
  data_path: string
  language: string
  theme: string
  font_family: string
  font_size: number
  text_color_light: string
  text_color_dark: string
  launch_at_login?: boolean
}

export const useSettingsStore = defineStore('settings', {
  state: () => ({
    config: {
      data_path: '',
      language: 'zh-CN',
      theme: 'light',
      font_family: 'Arial',
      font_size: 14,
      text_color_light: '#333333',
      text_color_dark: '#e5e5e5',
      launch_at_login: false,
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
      this.config = await invoke('get_app_config')
      if (this.config.launch_at_login == null) this.config.launch_at_login = false
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

import * as ElementPlusIconsVue from '@element-plus/icons-vue'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import { createPinia } from 'pinia'
import { createApp } from 'vue'
import App from './App.vue'
import i18n from './i18n'
import router from './router'
import './style.css'

const app = createApp(App)
const pinia = createPinia()

// 打包后白屏时捕获错误并显示在页面上，便于排查
app.config.errorHandler = (err: unknown) => {
  const msg = err instanceof Error ? err.message : String(err)
  const stack = err instanceof Error ? err.stack : ''
  const el = document.createElement('div')
  el.id = 'app-error-overlay'
  el.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,0.9);color:#fff;padding:20px;font-family:monospace;font-size:12px;overflow:auto;z-index:99999;white-space:pre-wrap;'
  el.textContent = `[Error] ${msg}\n\n${stack}`
  document.body.appendChild(el)
  console.error(err)
}

for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component)
}

app.use(pinia)
app.use(router)
app.use(i18n)
app.use(ElementPlus)
app.mount('#app')

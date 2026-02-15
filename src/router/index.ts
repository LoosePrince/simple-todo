import { createRouter, createWebHashHistory } from 'vue-router'
import TodoListView from '../views/TodoListView.vue'
import TodoDetailView from '../views/TodoDetailView.vue'
import SettingsView from '../views/SettingsView.vue'

// 使用 Hash 模式：Tauri 打包后无论用何种 URL 加载，路径都由 # 后决定，避免 path 不匹配导致白屏
const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', name: 'Home', component: TodoListView },
    { path: '/detail/:id', name: 'Detail', component: TodoDetailView },
    { path: '/settings', name: 'Settings', component: SettingsView }
  ]
})

export default router

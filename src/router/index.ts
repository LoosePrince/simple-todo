import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'Home',
      component: () => import('../views/TodoListView.vue')
    },
    {
      path: '/detail/:id',
      name: 'Detail',
      component: () => import('../views/TodoDetailView.vue')
    },
    {
      path: '/settings',
      name: 'Settings',
      component: () => import('../views/SettingsView.vue')
    }
  ]
})

export default router

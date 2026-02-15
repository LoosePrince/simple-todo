import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'

export interface TodoItem {
  id: string
  title: string
  status: 'pending' | 'completed'
  folder_name: string
}

export const useTodoStore = defineStore('todo', {
  state: () => ({
    todos: [] as TodoItem[],
    dataPath: '',
  }),
  actions: {
    async loadTodos(dataPath: string) {
      this.dataPath = dataPath
      this.todos = await invoke('get_todos', { dataPath })
    },
    async addTodo(title: string) {
      const folder_name = await invoke<string>('create_todo_folder', { dataPath: this.dataPath })
      const newTodo: TodoItem = {
        id: crypto.randomUUID(),
        title,
        status: 'pending',
        folder_name,
      }
      this.todos.push(newTodo)
      await this.saveTodos()
    },
    async toggleTodo(id: string) {
      const todo = this.todos.find(t => t.id === id)
      if (todo) {
        todo.status = todo.status === 'pending' ? 'completed' : 'pending'
        await this.saveTodos()
      }
    },
    async deleteTodo(id: string) {
      this.todos = this.todos.filter(t => t.id !== id)
      await this.saveTodos()
    },
    async saveTodos() {
      await invoke('save_todos', { dataPath: this.dataPath, todos: this.todos })
    }
  }
})

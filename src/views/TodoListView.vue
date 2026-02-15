<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useTodoStore } from '../store/todo'
import { useSettingsStore } from '../store/settings'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Settings, Trash2, Plus } from 'lucide-vue-next'

const { t } = useI18n()
const todoStore = useTodoStore()
const settingsStore = useSettingsStore()
const router = useRouter()
const newTodoTitle = ref('')

onMounted(async () => {
  try {
    await settingsStore.loadConfig()
  } catch (_) {}
  try {
    await todoStore.loadTodos(settingsStore.config.data_path)
  } catch (_) {}
})

const handleAddTodo = async () => {
  if (newTodoTitle.value.trim()) {
    await todoStore.addTodo(newTodoTitle.value)
    newTodoTitle.value = ''
  }
}

const goToDetail = (id: string) => {
  router.push(`/detail/${id}`)
}
</script>

<template>
  <div class="todo-list-view">
    <div class="header">
      <h1>{{ t('todo.listTitle') }}</h1>
      <el-button circle @click="router.push('/settings')">
        <Settings :size="20" />
      </el-button>
    </div>

    <div class="input-section">
      <el-input
        v-model="newTodoTitle"
        :placeholder="t('todo.placeholder')"
        @keyup.enter="handleAddTodo"
      >
        <template #append>
          <el-button @click="handleAddTodo">
            <Plus :size="16" style="margin-right: 4px" />
            {{ t('common.add') }}
          </el-button>
        </template>
      </el-input>
    </div>

    <el-scrollbar class="list-section">
      <div v-for="item in todoStore.todos" :key="item.id" class="todo-item">
        <el-checkbox
          v-model="item.status"
          true-value="completed"
          false-value="pending"
          @change="todoStore.saveTodos()"
        />
        <span
          class="title"
          :class="{ completed: item.status === 'completed' }"
          @click="goToDetail(item.id)"
        >
          {{ item.title }}
        </span>
        <el-button
          type="danger"
          circle
          size="small"
          @click="todoStore.deleteTodo(item.id)"
        >
          <Trash2 :size="14" />
        </el-button>
      </div>
    </el-scrollbar>
  </div>
</template>

<style scoped>
.todo-list-view {
  padding: 20px;
  max-width: max(600px, 80%);
  margin: 0 auto;
  height: 100%;
  display: flex;
  flex-direction: column;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.input-section {
  margin-bottom: 20px;
}

.list-section {
  flex: 1;
}

.todo-item {
  display: flex;
  align-items: center;
  padding: 10px;
  border-bottom: 1px solid #eee;
  gap: 10px;
}

.title {
  flex: 1;
  cursor: pointer;
}

.title.completed {
  text-decoration: line-through;
  color: #999;
}
</style>

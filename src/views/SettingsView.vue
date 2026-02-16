<script setup lang="ts">
import { useSettingsStore } from '../store/settings'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { useI18n } from 'vue-i18n'
import { ChevronLeft, FolderOpen, Search, Trash2 } from 'lucide-vue-next'
import { ElMessage, ElMessageBox } from 'element-plus'
import { ref } from 'vue'

const { t } = useI18n()
const settingsStore = useSettingsStore()
const router = useRouter()

interface OrphanFolder {
  folder_name: string
  size: number
}

const orphanTodosVisible = ref(false)
const orphanTodos = ref<OrphanFolder[]>([])
const orphanTodosLoading = ref(false)

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`
}

async function findOrphanTodos() {
  orphanTodosLoading.value = true
  try {
    const result = await invoke<OrphanFolder[]>('find_orphan_todo_folders', {
      dataPath: settingsStore.config.data_path
    })
    orphanTodos.value = result
    orphanTodosVisible.value = true
  } catch (e) {
    ElMessage.error(`查找失败: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    orphanTodosLoading.value = false
  }
}

async function deleteOrphanFolder(folderName: string) {
  try {
    await invoke('delete_todo_folder', {
      dataPath: settingsStore.config.data_path,
      folderName
    })
    orphanTodos.value = orphanTodos.value.filter(f => f.folder_name !== folderName)
    ElMessage.success(t('settings.orphanTodosDeleteSuccess'))
  } catch (e) {
    ElMessage.error(`${t('settings.orphanTodosDeleteError')}: ${e instanceof Error ? e.message : String(e)}`)
  }
}

async function deleteAllOrphanTodos() {
  try {
    await ElMessageBox.confirm(
      t('settings.orphanTodosDeleteConfirm'),
      t('settings.orphanTodosTitle'),
      {
        type: 'warning',
        confirmButtonText: t('common.confirm'),
        cancelButtonText: t('common.cancel')
      }
    )
    
    for (const folder of orphanTodos.value) {
      try {
        await invoke('delete_todo_folder', {
          dataPath: settingsStore.config.data_path,
          folderName: folder.folder_name
        })
      } catch (e) {
        console.error(`删除文件夹失败: ${folder.folder_name}`, e)
      }
    }
    
    orphanTodos.value = []
    ElMessage.success(t('settings.orphanTodosDeleteSuccess'))
  } catch {
    // 用户取消
  }
}

let saveTimer: ReturnType<typeof setTimeout> | null = null
function scheduleSave() {
  if (saveTimer) clearTimeout(saveTimer)
  saveTimer = setTimeout(() => {
    saveTimer = null
    settingsStore.saveConfig()
  }, 300)
}

/** 主题/语言等会影响全局样式的项：先立即应用，再延迟持久化 */
function onThemeChange() {
  settingsStore.applyTheme()
  scheduleSave()
}
function onLanguageChange() {
  settingsStore.applyI18n()
  scheduleSave()
}

const themes = [
  { label: t('settings.themeLight'), value: 'light' },
  { label: t('settings.themeDark'), value: 'dark' }
]

const languages = [
  { label: '简体中文', value: 'zh-CN' },
  { label: 'English', value: 'en-US' }
]

const fontFamilies = [
  { label: '微软雅黑', value: 'Microsoft YaHei' },
  { label: '宋体', value: 'SimSun' },
  { label: 'Arial', value: 'Arial' },
  { label: 'Inter', value: 'Inter' }
]

async function onLaunchAtLoginChange(enabled: boolean) {
  try {
    const { enable, disable } = await import('@tauri-apps/plugin-autostart')
    if (enabled) await enable()
    else await disable()
    scheduleSave()
  } catch (e) {
    console.error('Autostart failed:', e)
  }
}

const handlePickFolder = async () => {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
    })
    if (selected) {
      const oldPath = settingsStore.config.data_path
      const newPath = selected as string
      
      // 只有在路径真正改变时才执行移动
      if (oldPath && oldPath !== newPath) {
        await invoke('move_data', { oldPath, newPath })
      }
      
      // 更新配置并立即保存
      settingsStore.config.data_path = newPath
      await settingsStore.saveConfig()
      
      // 重新加载代办列表以确保路径正确
      const todoStore = (await import('../store/todo')).useTodoStore()
      await todoStore.loadTodos(newPath)
    }
  } catch (e) {
    console.error('Failed to change data path:', e)
  }
}
</script>

<template>
  <div class="settings-view">
    <div class="header">
      <h2>{{ t('settings.title') }}</h2>
      <el-button @click="router.back()">
        <ChevronLeft :size="16" style="margin-right: 4px" />
        {{ t('common.back') }}
      </el-button>
    </div>

    <el-form :model="settingsStore.config" label-width="120px">
      <el-form-item :label="t('settings.language')">
        <el-select v-model="settingsStore.config.language" @change="onLanguageChange">
          <el-option v-for="item in languages" :key="item.value" :label="item.label" :value="item.value" />
        </el-select>
      </el-form-item>

      <el-form-item :label="t('settings.theme')">
        <el-radio-group v-model="settingsStore.config.theme" @change="onThemeChange">
          <el-radio v-for="item in themes" :key="item.value" :label="item.label" :value="item.value">{{ item.label }}</el-radio>
        </el-radio-group>
      </el-form-item>

      <el-form-item :label="t('settings.font')">
        <el-select v-model="settingsStore.config.font_family" @change="scheduleSave">
          <el-option v-for="item in fontFamilies" :key="item.value" :label="item.label" :value="item.value" />
        </el-select>
      </el-form-item>

      <el-form-item :label="t('settings.fontSize')">
        <el-input-number v-model="settingsStore.config.font_size" :min="12" :max="30" @change="scheduleSave" />
      </el-form-item>

      <el-form-item :label="t('settings.textColor')">
        <div class="color-pickers">
          <div class="color-picker-item">
            <span>浅色模式: </span>
            <el-color-picker v-model="settingsStore.config.text_color_light" @change="scheduleSave" />
          </div>
          <div class="color-picker-item">
            <span>深色模式: </span>
            <el-color-picker v-model="settingsStore.config.text_color_dark" @change="scheduleSave" />
          </div>
        </div>
      </el-form-item>

      <el-form-item :label="t('settings.launchAtLogin')">
        <div class="launch-at-login-row">
          <el-switch
            v-model="settingsStore.config.launch_at_login"
            :active-value="true"
            :inactive-value="false"
            @change="onLaunchAtLoginChange"
          />
          <span class="launch-at-login-desc">{{ t('settings.launchAtLoginDesc') }}</span>
        </div>
      </el-form-item>

      <el-form-item :label="t('settings.dataPath')">
        <el-input v-model="settingsStore.config.data_path" readonly>
          <template #append>
            <el-button @click="handlePickFolder">
              <FolderOpen :size="16" style="margin-right: 4px" />
              {{ t('settings.selectFolder') }}
            </el-button>
          </template>
        </el-input>
      </el-form-item>

      <el-form-item>
        <el-button type="warning" @click="findOrphanTodos" :loading="orphanTodosLoading">
          <Search :size="16" style="margin-right: 4px" />
          {{ t('settings.findOrphanTodos') }}
        </el-button>
      </el-form-item>
    </el-form>

    <el-dialog
      v-model="orphanTodosVisible"
      :title="t('settings.orphanTodosTitle')"
      width="600px"
    >
      <div v-if="orphanTodos.length === 0" class="orphan-empty">
        {{ t('settings.orphanTodosEmpty') }}
      </div>
      <div v-else>
        <div class="orphan-header">
          <span>{{ t('settings.orphanTodosSize') }}</span>
          <el-button type="danger" size="small" @click="deleteAllOrphanTodos">
            <Trash2 :size="14" style="margin-right: 4px" />
            {{ t('settings.orphanTodosDeleteAll') }}
          </el-button>
        </div>
        <el-scrollbar max-height="400px">
          <div class="orphan-list">
            <div v-for="folder in orphanTodos" :key="folder.folder_name" class="orphan-item">
              <div class="orphan-item-info">
                <span class="orphan-folder-name">{{ folder.folder_name }}</span>
                <span class="orphan-size">{{ formatSize(folder.size) }}</span>
              </div>
              <el-button type="danger" size="small" @click="deleteOrphanFolder(folder.folder_name)">
                <Trash2 :size="14" />
              </el-button>
            </div>
          </div>
        </el-scrollbar>
      </div>
    </el-dialog>
  </div>
</template>

<style scoped>
.settings-view {
  padding: 20px;
  max-width: max(600px, 80%);
  margin: 0 auto;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 30px;
}

.color-pickers {
  display: flex;
  gap: 20px;
}

.color-picker-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.color-picker-item span {
  font-size: 12px;
  color: #666;
}

.launch-at-login-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.launch-at-login-desc {
  font-size: 12px;
  color: #666;
}

.dark .launch-at-login-desc {
  color: #aaa;
}

.dark .color-picker-item span {
  color: #aaa;
}

.orphan-empty {
  text-align: center;
  padding: 40px 0;
  color: #999;
}

.orphan-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid rgba(0, 0, 0, 0.1);
}

.dark .orphan-header {
  border-bottom-color: rgba(255, 255, 255, 0.1);
}

.orphan-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.orphan-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px;
  background: rgba(0, 0, 0, 0.02);
  border-radius: 6px;
  border: 1px solid rgba(0, 0, 0, 0.05);
}

.dark .orphan-item {
  background: rgba(255, 255, 255, 0.05);
  border-color: rgba(255, 255, 255, 0.1);
}

.orphan-item-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
  min-width: 0;
}

.orphan-folder-name {
  font-size: 13px;
  font-family: monospace;
  word-break: break-all;
  color: var(--app-text-color);
}

.orphan-size {
  font-size: 12px;
  color: #666;
}

.dark .orphan-size {
  color: #aaa;
}
</style>

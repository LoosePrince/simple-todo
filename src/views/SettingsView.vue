<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { ElMessage, ElMessageBox } from 'element-plus'
import { ChevronLeft, Cloud, FolderOpen, Keyboard, Search, Trash2 } from 'lucide-vue-next'
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useSettingsStore } from '../store/settings'
import { useSyncStore } from '../store/sync'

type WebDavConnectionResult = {
  ok: boolean
  status: string
  message: string
}

type PrepareSyncResult = {
  local_file_count: number
  local_manifest_path: string
  remote_manifest_url: string
  remote_initialized: boolean
}

type SyncAction = 'keep_local' | 'use_remote' | 'duplicate_remote' | 'mark_resolved' | 'later'

const { t } = useI18n()
const settingsStore = useSettingsStore()
const syncStore = useSyncStore()
const router = useRouter()

interface OrphanFolder {
  folder_name: string
  size: number
}

const orphanTodosVisible = ref(false)
const orphanTodos = ref<OrphanFolder[]>([])
const orphanTodosLoading = ref(false)
const orphanHelpVisible = ref(false)
const webdavPassword = ref('')
const webdavTesting = ref(false)
const webdavCredentialSaving = ref(false)
const webdavCredentialClearing = ref(false)
const webdavConnectionStatus = ref<WebDavConnectionResult | null>(null)
const syncPreparing = ref(false)
const syncNowLoading = ref(false)
const conflictResolving = ref<Record<string, boolean>>({})
const prepareSyncResult = ref<PrepareSyncResult | null>(null)

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
    ElMessage.error(`${t('settings.orphanTodosFindError')}: ${e instanceof Error ? e.message : String(e)}`)
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

const themes = computed(() => [
  { label: t('settings.themeLight'), value: 'light' },
  { label: t('settings.themeDark'), value: 'dark' }
])

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

const syncConflictOptions = computed(() => [
  { label: t('sync.conflictAsk'), value: 'ask' },
  { label: t('sync.conflictKeepLocal'), value: 'keep_local' },
  { label: t('sync.conflictUseRemote'), value: 'use_remote' },
  { label: t('sync.conflictDuplicateRemote'), value: 'duplicate_remote' }
])

function webdavPayload() {
  return {
    webdav_url: settingsStore.config.webdav_url || '',
    webdav_username: settingsStore.config.webdav_username || '',
    webdav_remote_dir: settingsStore.config.webdav_remote_dir || '/simple-todo',
  }
}

async function saveWebdavSettings() {
  try {
    await settingsStore.saveConfig()
    ElMessage.success(t('sync.settingsSaved'))
  } catch (e) {
    ElMessage.error(`${t('common.saveFailed')}: ${e instanceof Error ? e.message : String(e)}`)
  }
}

async function saveWebdavCredentials() {
  webdavCredentialSaving.value = true
  try {
    await invoke('save_webdav_credentials', {
      input: {
        webdav_url: settingsStore.config.webdav_url || '',
        webdav_username: settingsStore.config.webdav_username || '',
        password: webdavPassword.value
      }
    })
    webdavPassword.value = ''
    ElMessage.success(t('sync.credentialSaved'))
  } catch (e) {
    ElMessage.error(`${t('sync.credentialSaveFailed')}: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    webdavCredentialSaving.value = false
  }
}

async function clearWebdavCredentials() {
  webdavCredentialClearing.value = true
  try {
    await invoke('clear_webdav_credentials', {
      webdavUrl: settingsStore.config.webdav_url || '',
      webdavUsername: settingsStore.config.webdav_username || ''
    })
    webdavPassword.value = ''
    webdavConnectionStatus.value = null
    ElMessage.success(t('sync.credentialCleared'))
  } catch (e) {
    ElMessage.error(`${t('sync.credentialClearFailed')}: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    webdavCredentialClearing.value = false
  }
}

async function testWebdavConnection() {
  webdavTesting.value = true
  try {
    const result = await invoke<WebDavConnectionResult>('test_webdav_connection', {
      input: {
        ...webdavPayload(),
        password: webdavPassword.value || null
      }
    })
    webdavConnectionStatus.value = result
    if (result.ok) ElMessage.success(t('sync.connectionReady'))
    else ElMessage.warning(result.message)
  } catch (e) {
    webdavConnectionStatus.value = {
      ok: false,
      status: 'error',
      message: e instanceof Error ? e.message : String(e)
    }
    ElMessage.error(`${t('sync.connectionFailed')}: ${webdavConnectionStatus.value.message}`)
  } finally {
    webdavTesting.value = false
  }
}

async function prepareSyncManifest() {
  syncPreparing.value = true
  try {
    const result = await invoke<PrepareSyncResult>('prepare_sync_manifest', {
      input: {
        data_path: settingsStore.config.data_path,
        ...webdavPayload(),
        password: webdavPassword.value || null
      }
    })
    prepareSyncResult.value = result
    await syncStore.loadConflicts()
    ElMessage.success(t('sync.prepareSuccess', { count: result.local_file_count }))
  } catch (e) {
    ElMessage.error(`${t('sync.prepareFailed')}: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    syncPreparing.value = false
  }
}

async function syncNow() {
  syncNowLoading.value = true
  try {
    const result = await syncStore.syncNow(webdavPassword.value || null)
    if (!result) return
    ElMessage.success(t('sync.syncSuccess', {
      uploaded: result.uploaded_count,
      downloaded: result.downloaded_count,
      conflicts: result.conflict_count
    }))
  } catch (e) {
    ElMessage.error(`${t('sync.syncFailed')}: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    syncNowLoading.value = false
  }
}

async function resolveConflict(conflictId: string, action: SyncAction) {
  if (action === 'later') return
  conflictResolving.value[conflictId] = true
  try {
    await syncStore.resolveConflict(conflictId, action, webdavPassword.value || null)
    ElMessage.success(t('sync.conflictResolved'))
  } catch (e) {
    ElMessage.error(`${t('sync.conflictResolveFailed')}: ${e instanceof Error ? e.message : String(e)}`)
  } finally {
    conflictResolving.value[conflictId] = false
  }
}

function formatSyncTime(seconds?: number | null) {
  if (!seconds) return t('sync.neverSynced')
  return new Date(seconds * 1000).toLocaleString()
}

async function onSyncEnabledChange() {
  await saveWebdavSettings()
  syncStore.startAutoSync()
}

onMounted(() => {
  syncStore.loadConflicts().catch(() => {})
})

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
          <el-radio v-for="item in themes" :key="item.value" :label="item.label" :value="item.value">{{ item.label
            }}</el-radio>
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
            <span>{{ t('settings.textColorLight') }}: </span>
            <el-color-picker v-model="settingsStore.config.text_color_light" @change="scheduleSave" />
          </div>
          <div class="color-picker-item">
            <span>{{ t('settings.textColorDark') }}: </span>
            <el-color-picker v-model="settingsStore.config.text_color_dark" @change="scheduleSave" />
          </div>
        </div>
      </el-form-item>

      <el-form-item :label="t('settings.launchAtLogin')">
        <div class="launch-at-login-row">
          <el-switch v-model="settingsStore.config.launch_at_login" :active-value="true" :inactive-value="false"
            @change="onLaunchAtLoginChange" />
          <span class="launch-at-login-desc">{{ t('settings.launchAtLoginDesc') }}</span>
        </div>
      </el-form-item>

      <el-form-item :label="t('settings.shortcuts')">
        <el-button @click="router.push('/shortcuts')">
          <Keyboard :size="16" style="margin-right: 4px" />
          {{ t('settings.manageShortcuts') }}
        </el-button>
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

      <div class="settings-section-title">
        <Cloud :size="16" />
        <span>{{ t('sync.title') }}</span>
      </div>

      <el-form-item :label="t('sync.enabled')">
        <div class="sync-row">
          <el-switch v-model="settingsStore.config.sync_enabled" :active-value="true" :inactive-value="false"
            @change="onSyncEnabledChange" />
          <span class="sync-desc">{{ t('sync.enabledDesc') }}</span>
        </div>
      </el-form-item>

      <el-form-item :label="t('sync.webdavUrl')">
        <el-input v-model="settingsStore.config.webdav_url" :placeholder="t('sync.webdavUrlPlaceholder')" @change="saveWebdavSettings" />
      </el-form-item>

      <el-form-item :label="t('sync.remoteDir')">
        <el-input v-model="settingsStore.config.webdav_remote_dir" placeholder="/simple-todo" @change="saveWebdavSettings" />
      </el-form-item>

      <el-form-item :label="t('sync.username')">
        <el-input v-model="settingsStore.config.webdav_username" :placeholder="t('sync.usernamePlaceholder')" @change="saveWebdavSettings" />
      </el-form-item>

      <el-form-item :label="t('sync.password')">
        <el-input v-model="webdavPassword" type="password" show-password :placeholder="t('sync.passwordPlaceholder')" />
      </el-form-item>

      <el-form-item :label="t('sync.options')">
        <div class="sync-options">
          <el-checkbox v-model="settingsStore.config.sync_on_startup" @change="saveWebdavSettings">
            {{ t('sync.onStartup') }}
          </el-checkbox>
          <el-checkbox v-model="settingsStore.config.sync_on_change" @change="saveWebdavSettings">
            {{ t('sync.onChange') }}
          </el-checkbox>
          <el-input-number v-model="settingsStore.config.sync_interval_seconds" :min="60" :max="3600" :step="60"
            @change="saveWebdavSettings" />
          <span class="sync-desc">{{ t('sync.intervalSeconds') }}</span>
        </div>
      </el-form-item>

      <el-form-item :label="t('sync.conflictDefault')">
        <el-select v-model="settingsStore.config.sync_conflict_default_action" @change="saveWebdavSettings">
          <el-option v-for="item in syncConflictOptions" :key="item.value" :label="item.label" :value="item.value" />
        </el-select>
      </el-form-item>

      <el-form-item :label="t('sync.connection')">
        <div class="sync-actions">
          <el-button type="primary" @click="testWebdavConnection" :loading="webdavTesting">
            {{ t('sync.testConnection') }}
          </el-button>
          <el-button @click="saveWebdavCredentials" :loading="webdavCredentialSaving">
            {{ t('sync.saveCredential') }}
          </el-button>
          <el-button @click="clearWebdavCredentials" :loading="webdavCredentialClearing">
            {{ t('sync.clearCredential') }}
          </el-button>
          <el-button @click="prepareSyncManifest" :loading="syncPreparing">
            {{ t('sync.prepareSync') }}
          </el-button>
          <el-button type="success" @click="syncNow" :loading="syncNowLoading || syncStore.syncing">
            {{ t('sync.syncNow') }}
          </el-button>
          <span v-if="webdavConnectionStatus" class="sync-status" :class="{ ok: webdavConnectionStatus.ok }">
            {{ webdavConnectionStatus.ok ? t('sync.connectionReady') : webdavConnectionStatus.message }}
          </span>
          <span v-if="prepareSyncResult" class="sync-status ok">
            {{ t('sync.prepareReady', { count: prepareSyncResult.local_file_count }) }}
          </span>
        </div>
      </el-form-item>

      <el-form-item :label="t('sync.status')">
        <div class="sync-status-panel">
          <el-tag :type="syncStore.phase === 'error' ? 'danger' : syncStore.phase === 'conflict' ? 'warning' : syncStore.phase === 'success' ? 'success' : 'info'">
            {{ t(`sync.phase.${syncStore.phase}`) }}
          </el-tag>
          <span class="sync-desc">
            {{ t('sync.lastSyncAt') }}: {{ formatSyncTime(syncStore.lastResult?.last_sync_at) }}
          </span>
          <span v-if="syncStore.lastResult" class="sync-desc">
            {{ t('sync.lastSummary', {
              uploaded: syncStore.lastResult.uploaded_count,
              downloaded: syncStore.lastResult.downloaded_count,
              skipped: syncStore.lastResult.skipped_count,
              conflicts: syncStore.lastResult.conflict_count
            }) }}
          </span>
          <span v-if="syncStore.lastError" class="sync-status">
            {{ syncStore.lastError }}
          </span>
        </div>
      </el-form-item>

      <el-form-item :label="t('sync.conflicts')">
        <div class="conflict-inbox">
          <div v-if="syncStore.conflicts.length === 0" class="sync-desc">
            {{ t('sync.noConflicts') }}
          </div>
          <div v-for="conflict in syncStore.conflicts" :key="conflict.id" class="conflict-item">
            <div class="conflict-info">
              <strong>{{ conflict.path }}</strong>
              <span>{{ t(`sync.reason.${conflict.reason}`) }}</span>
              <small>{{ t('sync.detectedAt') }}: {{ formatSyncTime(conflict.created_at) }}</small>
            </div>
            <div class="conflict-actions">
              <el-button size="small" :loading="conflictResolving[conflict.id]" @click="resolveConflict(conflict.id, 'keep_local')">
                {{ t('sync.actionKeepLocal') }}
              </el-button>
              <el-button size="small" type="primary" :loading="conflictResolving[conflict.id]" @click="resolveConflict(conflict.id, 'use_remote')">
                {{ t('sync.actionUseRemote') }}
              </el-button>
              <el-button size="small" type="warning" :loading="conflictResolving[conflict.id]" @click="resolveConflict(conflict.id, 'duplicate_remote')">
                {{ t('sync.actionDuplicateRemote') }}
              </el-button>
              <el-button size="small" :loading="conflictResolving[conflict.id]" @click="resolveConflict(conflict.id, 'mark_resolved')">
                {{ t('sync.actionMarkResolved') }}
              </el-button>
              <el-button size="small" text @click="resolveConflict(conflict.id, 'later')">
                {{ t('sync.actionLater') }}
              </el-button>
            </div>
          </div>
        </div>
      </el-form-item>

      <el-form-item :label="t('settings.findOrphanTodos')">
        <div class="orphan-todos-actions">
          <el-button @click="findOrphanTodos" :loading="orphanTodosLoading">
            <Search :size="16" style="margin-right: 4px" />
            {{ t('common.search') }}
          </el-button>
          <el-button class="orphan-help-btn" text @click="orphanHelpVisible = true">
            ?
          </el-button>
        </div>
      </el-form-item>
    </el-form>

    <el-dialog v-model="orphanTodosVisible" :title="t('settings.orphanTodosTitle')" width="600px">
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
    <el-dialog v-model="orphanHelpVisible" :title="t('settings.orphanHelpTitle')" width="500px">
      <p>
        {{ t('settings.orphanHelpDesc1') }}
      </p>
      <p style="margin-top: 8px;">
        {{ t('settings.orphanHelpDesc2') }}
      </p>
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

.launch-at-login-row,
.sync-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.launch-at-login-desc,
.sync-desc {
  font-size: 12px;
  color: #666;
}

.dark .launch-at-login-desc,
.dark .sync-desc {
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

.orphan-todos-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.settings-section-title {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 28px 0 18px;
  padding-top: 18px;
  border-top: 1px solid rgba(0, 0, 0, 0.08);
  font-weight: 600;
}

.dark .settings-section-title {
  border-top-color: rgba(255, 255, 255, 0.12);
}

.sync-options,
.sync-actions,
.sync-status-panel {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
}

.sync-status-panel {
  align-items: flex-start;
  flex-direction: column;
}

.conflict-inbox {
  display: flex;
  flex-direction: column;
  gap: 10px;
  width: 100%;
}

.conflict-item {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  padding: 12px;
  border: 1px solid var(--app-border-color);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.02);
}

.dark .conflict-item {
  background: rgba(255, 255, 255, 0.05);
}

.conflict-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
  word-break: break-all;
}

.conflict-info small {
  color: #888;
}

.conflict-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
}

.sync-status {
  font-size: 12px;
  color: #d97706;
}

.sync-status.ok {
  color: #16a34a;
}

.orphan-help-btn {
  padding: 0 6px;
  font-size: 14px;
}
</style>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch, type ComponentPublicInstance } from 'vue'
import { useI18n } from 'vue-i18n'

const props = defineProps<{
  visible: boolean
  x: number
  y: number
  allowAssets?: boolean
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'action', action: string, value?: string): void
}>()

const { t } = useI18n()
const menuRef = ref<HTMLElement | null>(null)
const submenuRef = ref<HTMLElement | null>(null)
const groupRefs = new Map<string, HTMLElement>()
const position = ref({ left: 0, top: 0 })
const menuMaxHeight = ref(0)
const activeGroupKey = ref<string | null>(null)
const submenuPosition = ref({ left: 0, top: 0 })
const submenuMaxHeight = ref(0)
const EDGE_PAD = 8
const SUBMENU_GAP = 4

type MenuItem = {
  label: string
  action: string
  value?: string
}

type MenuGroup = {
  key: string
  label: string
  items: MenuItem[]
}

const menuGroups = computed<MenuGroup[]>(() => {
  const assetItems: MenuItem[] = props.allowAssets ? [
    { label: t('editor.toolbarInsertImage'), action: 'insertImage' },
    { label: t('editor.toolbarInsertFile'), action: 'insertFile' }
  ] : []

  return [
    {
      key: 'format',
      label: t('editor.contextFormat'),
      items: [
        { label: t('editor.toolbarBold'), action: 'bold' },
        { label: t('editor.toolbarItalic'), action: 'italic' },
        { label: t('editor.toolbarList'), action: 'insertUnorderedList' }
      ]
    },
    {
      key: 'block',
      label: t('editor.contextBlock'),
      items: [
        { label: t('editor.toolbarParagraph'), action: 'formatBlock', value: 'p' },
        { label: t('editor.toolbarHeading1'), action: 'formatBlock', value: 'h1' },
        { label: t('editor.toolbarHeading2'), action: 'formatBlock', value: 'h2' }
      ]
    },
    {
      key: 'align',
      label: t('editor.contextAlign'),
      items: [
        { label: t('editor.toolbarAlignLeft'), action: 'justifyLeft' },
        { label: t('editor.toolbarAlignCenter'), action: 'justifyCenter' },
        { label: t('editor.toolbarAlignRight'), action: 'justifyRight' }
      ]
    },
    {
      key: 'insert',
      label: t('editor.contextInsert'),
      items: [
        ...assetItems,
        { label: t('editor.toolbarInsertTask'), action: 'insertTask' },
        { label: t('editor.toolbarInsertCode'), action: 'insertCode' },
        { label: t('editor.toolbarInsertMarkdown'), action: 'insertMarkdown' },
        { label: t('editor.toolbarInsertCanvas'), action: 'insertCanvas' },
        { label: t('editor.toolbarInsertFold'), action: 'insertFold' }
      ]
    },
    {
      key: 'history',
      label: t('editor.contextHistory'),
      items: [
        { label: t('editor.toolbarUndo'), action: 'undo' },
        { label: t('editor.toolbarRedo'), action: 'redo' }
      ]
    }
  ]
})

function placeMainMenu() {
  const rect = menuRef.value?.getBoundingClientRect()
  if (!rect) return

  const maxHeight = Math.max(120, window.innerHeight - EDGE_PAD * 2)
  let left = props.x
  let top = props.y

  if (left + rect.width > window.innerWidth - EDGE_PAD) left = window.innerWidth - rect.width - EDGE_PAD
  if (top + rect.height > window.innerHeight - EDGE_PAD) top = window.innerHeight - Math.min(rect.height, maxHeight) - EDGE_PAD

  position.value = {
    left: Math.max(EDGE_PAD, left),
    top: Math.max(EDGE_PAD, top)
  }
  menuMaxHeight.value = maxHeight
}

function placeSubmenu(groupKey: string) {
  const groupEl = groupRefs.get(groupKey)
  if (groupEl) {
    const groupRect = groupEl.getBoundingClientRect()
    const maxHeight = Math.max(96, window.innerHeight - EDGE_PAD * 2)
    submenuPosition.value = {
      left: Math.min(window.innerWidth - EDGE_PAD, groupRect.right + SUBMENU_GAP),
      top: Math.max(EDGE_PAD, Math.min(window.innerHeight - EDGE_PAD, groupRect.top - 4))
    }
    submenuMaxHeight.value = maxHeight
  }
  activeGroupKey.value = groupKey
  nextTick(() => {
    const groupEl = groupRefs.get(groupKey)
    const submenuEl = submenuRef.value
    if (!groupEl || !submenuEl) return

    const groupRect = groupEl.getBoundingClientRect()
    const submenuRect = submenuEl.getBoundingClientRect()
    const maxHeight = Math.max(96, window.innerHeight - EDGE_PAD * 2)
    const openRightLeft = groupRect.right + SUBMENU_GAP
    const openLeftLeft = groupRect.left - submenuRect.width - SUBMENU_GAP
    const hasRightSpace = openRightLeft + submenuRect.width <= window.innerWidth - EDGE_PAD
    let left = hasRightSpace || openLeftLeft < EDGE_PAD ? openRightLeft : openLeftLeft
    let top = groupRect.top - 4

    if (left + submenuRect.width > window.innerWidth - EDGE_PAD) left = window.innerWidth - submenuRect.width - EDGE_PAD
    if (top + submenuRect.height > window.innerHeight - EDGE_PAD) top = window.innerHeight - Math.min(submenuRect.height, maxHeight) - EDGE_PAD

    submenuPosition.value = {
      left: Math.max(EDGE_PAD, left),
      top: Math.max(EDGE_PAD, top)
    }
    submenuMaxHeight.value = maxHeight
  })
}

function clearSubmenu() {
  activeGroupKey.value = null
}

function setGroupRef(key: string, el: Element | ComponentPublicInstance | null) {
  if (el instanceof HTMLElement) {
    groupRefs.set(key, el)
    return
  }
  groupRefs.delete(key)
}

function handlePointerMove(event: PointerEvent) {
  if (!activeGroupKey.value) return
  const target = event.target
  if (!(target instanceof Node)) return
  if (menuRef.value?.contains(target) || submenuRef.value?.contains(target)) return
  clearSubmenu()
}

watch(() => props.visible, (visible) => {
  if (!visible) {
    clearSubmenu()
    document.removeEventListener('pointermove', handlePointerMove)
    return
  }
  position.value = { left: props.x, top: props.y }
  menuMaxHeight.value = Math.max(120, window.innerHeight - EDGE_PAD * 2)
  document.addEventListener('pointermove', handlePointerMove)
  nextTick(placeMainMenu)
})

onBeforeUnmount(() => {
  document.removeEventListener('pointermove', handlePointerMove)
})

function run(action: string, value?: string) {
  clearSubmenu()
  emit('action', action, value)
}
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="editor-context-overlay" @click="emit('close')" @contextmenu.prevent="emit('close')">
      <div
        ref="menuRef"
        class="editor-context-menu"
        :style="{ left: position.left + 'px', top: position.top + 'px', maxHeight: menuMaxHeight + 'px' }"
        @click.stop
        @contextmenu.prevent
      >
        <div
          v-for="group in menuGroups"
          :key="group.key"
          :ref="(el) => setGroupRef(group.key, el)"
          class="editor-context-group"
          @mouseenter="placeSubmenu(group.key)"
        >
          <button type="button" class="editor-context-item editor-context-parent" :class="{ active: activeGroupKey === group.key }">
            <span>{{ group.label }}</span>
            <span class="editor-context-arrow">›</span>
          </button>
        </div>
      </div>

      <div
        v-if="activeGroupKey"
        ref="submenuRef"
        class="editor-context-submenu"
        :style="{ left: submenuPosition.left + 'px', top: submenuPosition.top + 'px', maxHeight: submenuMaxHeight + 'px' }"
        @click.stop
      >
        <button
          v-for="item in menuGroups.find(group => group.key === activeGroupKey)?.items || []"
          :key="`${item.action}-${item.value || ''}`"
          type="button"
          class="editor-context-item"
          @click="run(item.action, item.value)"
        >
          {{ item.label }}
        </button>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.editor-context-overlay {
  position: fixed;
  inset: 0;
  z-index: 10020;
}

.editor-context-menu,
.editor-context-submenu {
  min-width: 168px;
  padding: 4px;
  overflow-y: auto;
  overscroll-behavior: contain;
  background: var(--app-bg-color);
  border: 1px solid var(--app-border-color);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.16);
}

.editor-context-menu::-webkit-scrollbar,
.editor-context-submenu::-webkit-scrollbar {
  width: 6px;
}

.editor-context-menu::-webkit-scrollbar-thumb,
.editor-context-submenu::-webkit-scrollbar-thumb {
  background: rgba(128, 128, 128, 0.36);
  border-radius: 999px;
}

.dark .editor-context-menu,
.dark .editor-context-submenu {
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.42);
}

.editor-context-menu {
  position: fixed;
  left: 0;
  top: 0;
}

.editor-context-group {
  position: relative;
}

.editor-context-submenu {
  position: fixed;
  left: 0;
  top: 0;
  z-index: 10021;
}

.editor-context-parent.active {
  background: rgba(0, 0, 0, 0.06);
}

.editor-context-item {
  width: 100%;
  min-height: 30px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  padding: 6px 10px;
  font-size: 13px;
  font-family: var(--app-font-family);
  color: var(--app-text-color);
  background: transparent;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  white-space: nowrap;
  text-align: left;
}

.editor-context-item:hover {
  background: rgba(0, 0, 0, 0.06);
}

.dark .editor-context-item:hover,
.dark .editor-context-parent.active {
  background: rgba(255, 255, 255, 0.1);
}

.editor-context-arrow {
  opacity: 0.55;
}
</style>
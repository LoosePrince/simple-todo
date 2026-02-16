<script setup lang="ts">
import { AlignCenter, AlignLeft, AlignRight, Bold, FilePlus, Image as ImageIcon, Italic, List, ListTodo, Redo2, Undo2 } from 'lucide-vue-next'
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
const emit = defineEmits(['command', 'insert-image', 'insert-file', 'insert-task', 'mousedown'])

const commands = [
  { icon: Bold, key: 'toolbarBold', cmd: 'bold' },
  { icon: Italic, key: 'toolbarItalic', cmd: 'italic' },
  { icon: List, key: 'toolbarList', cmd: 'insertUnorderedList' },
]

const alignCommands = [
  { icon: AlignLeft, key: 'toolbarAlignLeft', cmd: 'justifyLeft' },
  { icon: AlignCenter, key: 'toolbarAlignCenter', cmd: 'justifyCenter' },
  { icon: AlignRight, key: 'toolbarAlignRight', cmd: 'justifyRight' },
]

const blockStyles = [
  { label: 'P', value: 'p', key: 'toolbarParagraph' },
  { label: 'H1', value: 'h1', key: 'toolbarHeading1' },
  { label: 'H2', value: 'h2', key: 'toolbarHeading2' },
]

const bubbleMenuVisible = ref(false)
const bubbleMenuPos = ref({ top: 0, left: 0 })
const bubbleColorValue = ref('#000000')
const isColorPickerOpen = ref(false)
const savedBubbleSelection = ref<Range | null>(null)
const DEFAULT_TEXT_COLOR = '#000000'
const BUBBLE_MENU_PADDING = 8
const BUBBLE_MENU_APPROX_WIDTH = 220
const BUBBLE_MENU_APPROX_HEIGHT = 36

/** 获取当前选中文本的颜色 */
function getSelectedTextColor(): string {
  const selection = window.getSelection()
  if (!selection || selection.rangeCount === 0) return DEFAULT_TEXT_COLOR
  
  const range = selection.getRangeAt(0)
  if (range.collapsed) return DEFAULT_TEXT_COLOR
  
  // 尝试从选中文本的父元素获取颜色
  let node: Node | null = range.commonAncestorContainer
  if (node.nodeType === Node.TEXT_NODE) {
    node = node.parentElement
  }
  
  let element = node as HTMLElement | null
  while (element) {
    const color = window.getComputedStyle(element).color
    if (color && color !== 'rgba(0, 0, 0, 0)' && color !== 'transparent' && color !== 'rgb(0, 0, 0)') {
      // 将 rgb/rgba 转换为 hex
      const rgbMatch = color.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/)
      if (rgbMatch) {
        const r = parseInt(rgbMatch[1]).toString(16).padStart(2, '0')
        const g = parseInt(rgbMatch[2]).toString(16).padStart(2, '0')
        const b = parseInt(rgbMatch[3]).toString(16).padStart(2, '0')
        return `#${r}${g}${b}`.toUpperCase()
      }
      // 如果已经是 hex 格式
      if (color.startsWith('#')) return color.toUpperCase()
    }
    element = element.parentElement
  }
  
  return DEFAULT_TEXT_COLOR
}

const updateBubbleMenu = () => {
  // 如果颜色选择器打开，不关闭工具栏
  if (isColorPickerOpen.value) {
    return
  }
  
  // 检查点击是否在工具栏或颜色选择器内
  const activeElement = document.activeElement
  if (activeElement && (activeElement.closest('.bubble-menu') || activeElement.closest('.el-color-picker') || activeElement.closest('.el-color-picker__panel'))) {
    return
  }
  
  const selection = window.getSelection()
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) {
    // 检查是否有颜色选择器相关的元素在焦点
    const pickerPanel = document.querySelector('.el-color-picker__panel')
    if (pickerPanel && pickerPanel.contains(activeElement)) {
      return
    }
    bubbleMenuVisible.value = false
    return
  }
  const range = selection.getRangeAt(0)
  const rect = range.getBoundingClientRect()
  const vw = window.innerWidth
  const scrollY = window.scrollY
  const scrollX = window.scrollX
  let left = rect.left + rect.width / 2 - BUBBLE_MENU_APPROX_WIDTH / 2
  let top = rect.top - BUBBLE_MENU_APPROX_HEIGHT - BUBBLE_MENU_PADDING
  if (top < 0) {
    top = rect.bottom + BUBBLE_MENU_PADDING
  }
  top += scrollY
  left += scrollX
  left = Math.max(BUBBLE_MENU_PADDING, Math.min(vw - BUBBLE_MENU_APPROX_WIDTH - BUBBLE_MENU_PADDING + scrollX, left))
  bubbleMenuPos.value = { top, left }
  // 更新颜色选择器显示当前选中文本的颜色
  bubbleColorValue.value = getSelectedTextColor()
  bubbleMenuVisible.value = true
}

function onBubbleColorChange(val: string | null) {
  // 空值时不执行命令（不保存颜色），但保持显示默认颜色
  if (val == null || val === '' || val === 'null') {
    bubbleColorValue.value = DEFAULT_TEXT_COLOR
    // 延迟重置，确保颜色选择器完全关闭
    setTimeout(() => {
      isColorPickerOpen.value = false
      // 恢复保存的选区
      if (savedBubbleSelection.value) {
        const sel = window.getSelection()
        if (sel) {
          sel.removeAllRanges()
          sel.addRange(savedBubbleSelection.value)
        }
      }
    }, 50)
    return
  }
  
  const color = val.startsWith('#') ? val : `#${val}`
  bubbleColorValue.value = color
  
  // 延迟重置，确保颜色选择器完全关闭后再恢复选区
  setTimeout(() => {
    isColorPickerOpen.value = false
    
    // 恢复保存的选区
    if (savedBubbleSelection.value) {
      const sel = window.getSelection()
      if (sel) {
        sel.removeAllRanges()
        sel.addRange(savedBubbleSelection.value)
      }
    }
    
    // 保存选区并执行命令
    emit('mousedown')
    emit('command', 'foreColor', color)
  }, 50)
}

function onBubbleColorPickerMouseDown() {
  // 点击颜色选择器时保存选区
  const selection = window.getSelection()
  if (selection && selection.rangeCount > 0 && !selection.isCollapsed) {
    savedBubbleSelection.value = selection.getRangeAt(0).cloneRange()
    isColorPickerOpen.value = true
    emit('mousedown')
  }
}

function onBubbleColorActiveChange(val: string | null) {
  // 颜色面板打开时标记状态
  if (val != null && !isColorPickerOpen.value) {
    const selection = window.getSelection()
    if (selection && selection.rangeCount > 0 && !selection.isCollapsed) {
      if (!savedBubbleSelection.value) {
        savedBubbleSelection.value = selection.getRangeAt(0).cloneRange()
      }
      isColorPickerOpen.value = true
      emit('mousedown')
    }
  } else if (val == null) {
    // 颜色面板关闭时，延迟重置标志（给 change 事件时间执行）
    setTimeout(() => {
      if (!isColorPickerOpen.value) {
        isColorPickerOpen.value = false
      }
    }, 100)
  }
}

// 监听颜色选择器面板的显示/隐藏
let colorPickerObserver: MutationObserver | null = null

onMounted(() => {
  document.addEventListener('selectionchange', updateBubbleMenu)
  
  // 监听颜色选择器面板的显示
  colorPickerObserver = new MutationObserver(() => {
    const pickerPanel = document.querySelector('.el-color-picker__panel')
    if (pickerPanel && pickerPanel.parentElement) {
      isColorPickerOpen.value = true
      // 保存当前选区
      const selection = window.getSelection()
      if (selection && selection.rangeCount > 0 && !selection.isCollapsed) {
        if (!savedBubbleSelection.value) {
          savedBubbleSelection.value = selection.getRangeAt(0).cloneRange()
        }
      }
    } else {
      // 面板关闭时延迟重置（给 change 事件时间执行）
      setTimeout(() => {
        if (!document.querySelector('.el-color-picker__panel')) {
          isColorPickerOpen.value = false
        }
      }, 100)
    }
  })
  
  colorPickerObserver.observe(document.body, {
    childList: true,
    subtree: true
  })
})

onBeforeUnmount(() => {
  document.removeEventListener('selectionchange', updateBubbleMenu)
  if (colorPickerObserver) {
    colorPickerObserver.disconnect()
    colorPickerObserver = null
  }
})
</script>

<template>
  <div class="editor-toolbar" @mousedown="emit('mousedown')">
    <div class="toolbar-group block-style-btns">
      <el-tooltip v-for="b in blockStyles" :key="b.value" :content="t('editor.' + b.key)" placement="top" popper-class="editor-tooltip-nohit">
        <button
          type="button"
          class="toolbar-style-btn"
          @mousedown="emit('mousedown')"
          @click="emit('command', 'formatBlock', b.value)"
        >{{ b.label }}</button>
      </el-tooltip>
    </div>

    <div class="toolbar-divider"></div>

    <div class="toolbar-group">
      <el-tooltip v-for="c in commands" :key="c.cmd" :content="t('editor.' + c.key)" placement="top" popper-class="editor-tooltip-nohit">
        <el-button circle @mousedown="emit('mousedown')" @click="emit('command', c.cmd)">
          <component :is="c.icon" :size="16" />
        </el-button>
      </el-tooltip>
      <el-tooltip :content="t('editor.toolbarInsertTask')" placement="top" popper-class="editor-tooltip-nohit">
        <el-button circle @mousedown="emit('mousedown')" @click="emit('insert-task')">
          <ListTodo :size="16" />
        </el-button>
      </el-tooltip>
    </div>

    <div class="toolbar-divider"></div>

    <div class="toolbar-group">
      <el-tooltip v-for="a in alignCommands" :key="a.cmd" :content="t('editor.' + a.key)" placement="top" popper-class="editor-tooltip-nohit">
        <el-button circle @mousedown="emit('mousedown')" @click="emit('command', a.cmd)">
          <component :is="a.icon" :size="16" />
        </el-button>
      </el-tooltip>
    </div>

    <div class="toolbar-divider"></div>

    <div class="toolbar-group">
      <el-tooltip :content="t('editor.toolbarUndo')" placement="top" popper-class="editor-tooltip-nohit">
        <el-button circle @mousedown="emit('mousedown')" @click="emit('command', 'undo')">
          <Undo2 :size="16" />
        </el-button>
      </el-tooltip>
      <el-tooltip :content="t('editor.toolbarRedo')" placement="top" popper-class="editor-tooltip-nohit">
        <el-button circle @mousedown="emit('mousedown')" @click="emit('command', 'redo')">
          <Redo2 :size="16" />
        </el-button>
      </el-tooltip>
    </div>

    <div class="toolbar-divider"></div>

    <div class="toolbar-group">
      <el-tooltip :content="t('editor.toolbarInsertImage')" placement="top" popper-class="editor-tooltip-nohit">
        <el-button circle @mousedown="emit('mousedown')" @click="emit('insert-image')">
          <ImageIcon :size="16" />
        </el-button>
      </el-tooltip>
      <el-tooltip :content="t('editor.toolbarInsertFile')" placement="top" popper-class="editor-tooltip-nohit">
        <el-button circle @mousedown="emit('mousedown')" @click="emit('insert-file')">
          <FilePlus :size="16" />
        </el-button>
      </el-tooltip>
    </div>

    <!-- 选择文本时的浮动小工具栏：mousedown 时保存选区，点击后由父组件恢复选区并执行命令 -->
    <Teleport to="body">
      <div
        v-if="bubbleMenuVisible"
        class="bubble-menu"
        :style="{ top: bubbleMenuPos.top + 'px', left: bubbleMenuPos.left + 'px' }"
        @mousedown="emit('mousedown')"
        @click.stop
      >
        <button type="button" class="bubble-btn" :title="t('editor.bubbleBold')" @click="emit('command', 'bold')">
          <Bold :size="14" />
        </button>
        <button type="button" class="bubble-btn" :title="t('editor.bubbleItalic')" @click="emit('command', 'italic')">
          <Italic :size="14" />
        </button>
        <el-tooltip :content="t('editor.bubbleColor')" placement="top" popper-class="editor-tooltip-nohit">
          <div class="bubble-color-wrapper" @mousedown.stop="onBubbleColorPickerMouseDown" @click.stop>
            <el-color-picker 
              size="small" 
              class="bubble-color" 
              color-format="hex" 
              :model-value="bubbleColorValue" 
              @change="onBubbleColorChange"
              @active-change="onBubbleColorActiveChange"
            />
          </div>
        </el-tooltip>
        <span class="bubble-divider" />
        <button type="button" class="bubble-btn" :title="t('editor.toolbarAlignLeft')" @click="emit('command', 'justifyLeft')">
          <AlignLeft :size="14" />
        </button>
        <button type="button" class="bubble-btn" :title="t('editor.toolbarAlignCenter')" @click="emit('command', 'justifyCenter')">
          <AlignCenter :size="14" />
        </button>
        <button type="button" class="bubble-btn" :title="t('editor.toolbarAlignRight')" @click="emit('command', 'justifyRight')">
          <AlignRight :size="14" />
        </button>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
:deep(.editor-tooltip-nohit) {
  pointer-events: none;
}

.editor-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  padding: 4px 8px;
  background: transparent;
  border: none;
  border-bottom: 1px solid rgba(0, 0, 0, 0.08);
  gap: 2px 6px;
  position: sticky;
  top: 0;
  z-index: 100;
  min-width: 0;
  transition: background 0.2s;
}

.editor-toolbar:hover {
  background: rgba(0, 0, 0, 0.04);
}

.dark .editor-toolbar {
  border-bottom-color: rgba(255, 255, 255, 0.08);
}

.dark .editor-toolbar:hover {
  background: rgba(255, 255, 255, 0.06);
}

.toolbar-group {
  display: flex;
  gap: 2px;
  flex-shrink: 0;
}

.toolbar-divider {
  width: 1px;
  height: 16px;
  margin: 0 2px;
  background: rgba(0,0,0,0.12);
  flex-shrink: 0;
}

@media (max-width: 480px) {
  .editor-toolbar {
    padding: 3px 6px;
    gap: 2px 4px;
  }
  .toolbar-style-btn {
    min-width: 26px;
    height: 24px;
    padding: 0 5px;
    font-size: 11px;
  }
}

.dark .toolbar-divider {
  background: rgba(255,255,255,0.1);
}

.toolbar-style-btn {
  min-width: 28px;
  height: 26px;
  padding: 0 6px;
  font-size: 12px;
  font-weight: 600;
  font-family: var(--app-font-family);
  color: var(--app-text-color);
  background: transparent;
  border: none;
  border-radius: 5px;
  cursor: pointer;
  transition: background 0.2s;
}

.toolbar-style-btn:hover {
  background: rgba(0, 0, 0, 0.08);
}

.dark .toolbar-style-btn:hover {
  background: rgba(255, 255, 255, 0.1);
}

.editor-toolbar :deep(.el-button.is-circle) {
  width: 28px;
  height: 28px;
  padding: 0;
  margin: 0;
  background: transparent;
  border: none;
  color: var(--app-text-color);
  transition: background 0.2s;
}

.editor-toolbar :deep(.el-button.is-circle:hover) {
  background: rgba(0, 0, 0, 0.08);
}

.dark .editor-toolbar :deep(.el-button.is-circle:hover) {
  background: rgba(255, 255, 255, 0.1);
}

.bubble-menu {
  position: absolute;
  background: var(--app-bg-color);
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.15);
  padding: 4px 6px;
  display: flex;
  align-items: center;
  gap: 4px;
  z-index: 1000;
  border: 1px solid rgba(0,0,0,0.1);
  backdrop-filter: blur(8px);
}

.dark .bubble-menu {
  border-color: rgba(255,255,255,0.1);
  background: rgba(30, 30, 30, 0.95);
}

.bubble-btn {
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  color: var(--app-text-color);
  background: transparent;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.2s;
}

.bubble-btn:hover {
  background: rgba(0, 0, 0, 0.08);
}

.dark .bubble-btn:hover {
  background: rgba(255, 255, 255, 0.1);
}

.bubble-color-wrapper {
  display: inline-flex;
  align-items: center;
  margin-left: 2px;
}


.bubble-divider {
  width: 1px;
  height: 18px;
  margin: 0 2px;
  background: rgba(0, 0, 0, 0.15);
  flex-shrink: 0;
}

.dark .bubble-divider {
  background: rgba(255, 255, 255, 0.15);
}
</style>

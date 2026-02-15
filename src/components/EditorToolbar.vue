<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import { Bold, Italic, List, Image as ImageIcon, FilePlus, AlignLeft, AlignCenter, AlignRight } from 'lucide-vue-next'

const emit = defineEmits(['command', 'insert-image', 'insert-file', 'mousedown'])

const commands = [
  { icon: Bold, label: '加粗', cmd: 'bold' },
  { icon: Italic, label: '倾斜', cmd: 'italic' },
  { icon: List, label: '列表', cmd: 'insertUnorderedList' },
]

const alignCommands = [
  { icon: AlignLeft, label: '左对齐', cmd: 'justifyLeft' },
  { icon: AlignCenter, label: '居中', cmd: 'justifyCenter' },
  { icon: AlignRight, label: '右对齐', cmd: 'justifyRight' },
]

const blockStyles = [
  { label: 'P', value: 'p', title: '正文' },
  { label: 'H1', value: 'h1', title: '标题' },
  { label: 'H2', value: 'h2', title: '副标题' },
]

const bubbleMenuVisible = ref(false)
const bubbleMenuPos = ref({ top: 0, left: 0 })
const BUBBLE_MENU_PADDING = 8
const BUBBLE_MENU_APPROX_WIDTH = 140
const BUBBLE_MENU_APPROX_HEIGHT = 36

const updateBubbleMenu = () => {
  const selection = window.getSelection()
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) {
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
  bubbleMenuVisible.value = true
}

onMounted(() => {
  document.addEventListener('selectionchange', updateBubbleMenu)
})

onBeforeUnmount(() => {
  document.removeEventListener('selectionchange', updateBubbleMenu)
})
</script>

<template>
  <div class="editor-toolbar" @mousedown="emit('mousedown')">
    <div class="toolbar-group block-style-btns">
      <button
        v-for="b in blockStyles"
        :key="b.value"
        type="button"
        class="toolbar-style-btn"
        :title="b.title"
        @mousedown="emit('mousedown')"
        @click="emit('command', 'formatBlock', b.value)"
      >{{ b.label }}</button>
    </div>

    <div class="toolbar-divider"></div>

    <div class="toolbar-group">
      <el-tooltip v-for="c in commands" :key="c.cmd" :content="c.label" placement="top">
        <el-button circle @mousedown="emit('mousedown')" @click="emit('command', c.cmd)">
          <component :is="c.icon" :size="16" />
        </el-button>
      </el-tooltip>
    </div>

    <div class="toolbar-divider"></div>

    <div class="toolbar-group">
      <el-tooltip v-for="a in alignCommands" :key="a.cmd" :content="a.label" placement="top">
        <el-button circle @mousedown="emit('mousedown')" @click="emit('command', a.cmd)">
          <component :is="a.icon" :size="16" />
        </el-button>
      </el-tooltip>
    </div>

    <div class="toolbar-divider"></div>

    <div class="toolbar-group">
      <el-button circle @click="emit('insert-image')">
        <ImageIcon :size="16" />
      </el-button>
      <el-button circle @click="emit('insert-file')">
        <FilePlus :size="16" />
      </el-button>
    </div>

    <!-- 选择文本时的浮动小工具栏：mousedown 时保存选区，点击后由父组件恢复选区并执行命令 -->
    <Teleport to="body">
      <div
        v-if="bubbleMenuVisible"
        class="bubble-menu"
        :style="{ top: bubbleMenuPos.top + 'px', left: bubbleMenuPos.left + 'px' }"
        @mousedown="emit('mousedown')"
      >
        <button type="button" class="bubble-btn" title="加粗" @click="emit('command', 'bold')">
          <Bold :size="14" />
        </button>
        <button type="button" class="bubble-btn" title="斜体" @click="emit('command', 'italic')">
          <Italic :size="14" />
        </button>
        <el-color-picker size="small" class="bubble-color" @change="(val: string) => emit('command', 'foreColor', val)" />
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.editor-toolbar {
  display: flex;
  align-items: center;
  padding: 8px 16px;
  background: var(--app-bg-color);
  border-bottom: 1px solid rgba(0,0,0,0.1);
  gap: 12px;
  position: sticky;
  top: 0;
  z-index: 100;
}

.dark .editor-toolbar {
  border-color: rgba(255,255,255,0.1);
}

.toolbar-group {
  display: flex;
  gap: 8px;
}

.toolbar-divider {
  width: 1px;
  height: 20px;
  background: rgba(0,0,0,0.1);
}

.dark .toolbar-divider {
  background: rgba(255,255,255,0.1);
}

.toolbar-style-btn {
  min-width: 32px;
  height: 28px;
  padding: 0 8px;
  font-size: 12px;
  font-weight: 600;
  font-family: var(--app-font-family);
  color: var(--app-text-color);
  background: rgba(0, 0, 0, 0.06);
  border: 1px solid rgba(0, 0, 0, 0.1);
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.2s, border-color 0.2s;
}

.toolbar-style-btn:hover {
  background: rgba(0, 0, 0, 0.1);
  border-color: rgba(0, 0, 0, 0.15);
}

.dark .toolbar-style-btn {
  background: rgba(255, 255, 255, 0.08);
  border-color: rgba(255, 255, 255, 0.15);
}

.dark .toolbar-style-btn:hover {
  background: rgba(255, 255, 255, 0.12);
  border-color: rgba(255, 255, 255, 0.2);
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

.bubble-color {
  margin-left: 2px;
}
</style>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'

export interface Block {
  id: string
  type: 'text' | 'h1' | 'h2' | 'image' | 'file'
  content: string
  bold?: boolean
  color?: string
  url?: string
  fileName?: string
}

const props = defineProps<{
  modelValue: Block[]
}>()

const emit = defineEmits(['update:modelValue', 'upload-image', 'upload-file'])

const blocks = ref<Block[]>([...props.modelValue])

if (blocks.value.length === 0) {
  blocks.value.push({ id: crypto.randomUUID(), type: 'text', content: '' })
}

watch(blocks, (newVal) => {
  emit('update:modelValue', newVal)
}, { deep: true })

const addBlock = (index: number, type: Block['type'] = 'text') => {
  const newBlock: Block = { id: crypto.randomUUID(), type, content: '' }
  blocks.value.splice(index + 1, 0, newBlock)
}

const removeBlock = (index: number) => {
  if (blocks.value.length > 1) {
    blocks.value.splice(index, 1)
  } else {
    blocks.value[0].content = ''
    blocks.value[0].type = 'text'
  }
}

const onImageUpload = (index: number) => {
  emit('upload-image', (url: string) => {
    blocks.value[index].type = 'image'
    blocks.value[index].url = url
  })
}

const onFileUpload = (index: number) => {
  emit('upload-file', (url: string, fileName: string) => {
    blocks.value[index].type = 'file'
    blocks.value[index].url = url
    blocks.value[index].fileName = fileName
  })
}
</script>

<template>
  <div class="block-editor">
    <div v-for="(block, index) in blocks" :key="block.id" class="block-item">
      <div class="block-controls">
        <el-dropdown trigger="click">
          <el-button icon="Plus" circle size="small" />
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item @click="addBlock(index, 'text')">正文</el-dropdown-item>
              <el-dropdown-item @click="addBlock(index, 'h1')">标题 1</el-dropdown-item>
              <el-dropdown-item @click="addBlock(index, 'h2')">标题 2</el-dropdown-item>
              <el-dropdown-item @click="onImageUpload(index)">图片</el-dropdown-item>
              <el-dropdown-item @click="onFileUpload(index)">文件</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <el-button icon="Delete" circle size="small" type="danger" @click="removeBlock(index)" />
      </div>

      <div class="block-content">
        <template v-if="block.type === 'text'">
          <el-input
            v-model="block.content"
            type="textarea"
            autosize
            placeholder="输入内容..."
            :style="{ fontWeight: block.bold ? 'bold' : 'normal', color: block.color }"
          />
          <div class="text-tools">
            <el-checkbox v-model="block.bold">加粗</el-checkbox>
            <el-color-picker v-model="block.color" size="small" />
          </div>
        </template>

        <template v-else-if="block.type === 'h1'">
          <el-input v-model="block.content" placeholder="一级标题" class="h1-input" />
        </template>

        <template v-else-if="block.type === 'h2'">
          <el-input v-model="block.content" placeholder="二级标题" class="h2-input" />
        </template>

        <template v-else-if="block.type === 'image'">
          <div class="image-block">
            <img :src="block.url" v-if="block.url" />
            <div v-else class="placeholder">图片加载中...</div>
          </div>
        </template>

        <template v-else-if="block.type === 'file'">
          <div class="file-block">
            <el-link :href="block.url" target="_blank" icon="Document">
              {{ block.fileName || '未知文件' }}
            </el-link>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.block-editor {
  display: flex;
  flex-direction: column;
  gap: 15px;
  padding: 10px;
}

.block-item {
  display: flex;
  gap: 10px;
  group: hover;
}

.block-controls {
  display: flex;
  flex-direction: column;
  gap: 5px;
  opacity: 0.2;
  transition: opacity 0.2s;
}

.block-item:hover .block-controls {
  opacity: 1;
}

.block-content {
  flex: 1;
}

.h1-input :deep(.el-input__inner) {
  font-size: 24px;
  font-weight: bold;
}

.h2-input :deep(.el-input__inner) {
  font-size: 20px;
  font-weight: bold;
}

.text-tools {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 5px;
}

.image-block img {
  max-width: 100%;
  border-radius: 4px;
}

.file-block {
  padding: 10px;
  background: rgba(0,0,0,0.05);
  border-radius: 4px;
}

.dark .file-block {
  background: rgba(255,255,255,0.1);
}
</style>

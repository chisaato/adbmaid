<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import {
  NButton,
  NCard,
  NEmpty,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NList,
  NListItem,
  NModal,
  NSelect,
  NSlider,
  NSpace,
  NSwitch,
  NTabPane,
  NTabs,
  NTag,
  useMessage,
} from 'naive-ui'
import { api } from '@/api/client'
import type { CommandSnippet, TerminalAppearance } from '@/api/types'
import TerminalPane from '@/components/TerminalPane.vue'
import { useWorkbenchStore } from '@/stores/workbench'

interface TermTab {
  id: string
  title: string
  serial: string
}

const store = useWorkbenchStore()
const message = useMessage()

const tabs = ref<TermTab[]>([])
const activeTabId = ref<string>('')
const paneRefs = new Map<string, InstanceType<typeof TerminalPane>>()

const snippetsOpen = ref(true)
const snippets = ref<CommandSnippet[]>([])
const newSnippetTitle = ref('')
const newSnippetCmd = ref('')

const appearanceOpen = ref(false)
const draftAppearance = ref<TerminalAppearance>({ ...store.terminalAppearance })

function setPaneRef(id: string, el: unknown) {
  if (el) {
    paneRefs.set(id, el as InstanceType<typeof TerminalPane>)
  } else {
    paneRefs.delete(id)
  }
}

function createTab() {
  if (!store.selectedDevice) {
    message.warning('请先选择一台在线设备')
    return
  }
  const id = `term-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`
  const label = store.selectedDevice.alias || store.selectedDevice.marketName || store.selectedDevice.serial
  tabs.value.push({
    id,
    title: `${label} #${tabs.value.length + 1}`,
    serial: store.selectedDevice.serial,
  })
  activeTabId.value = id
}

function closeTab(id: string) {
  const idx = tabs.value.findIndex((t) => t.id === id)
  if (idx < 0) return
  tabs.value.splice(idx, 1)
  if (activeTabId.value === id) {
    activeTabId.value = tabs.value[Math.max(0, idx - 1)]?.id ?? ''
  }
}

function clearCurrent() {
  paneRefs.get(activeTabId.value)?.clear()
}

function reconnectCurrent() {
  void paneRefs.get(activeTabId.value)?.reconnect()
}

function runSnippet(snip: CommandSnippet, autoEnter?: boolean) {
  const pane = paneRefs.get(activeTabId.value)
  if (!pane) {
    message.warning('请先打开一个终端标签页')
    return
  }
  pane.sendCommand(snip.command, autoEnter ?? snip.autoEnter)
}

async function addSnippet() {
  if (!newSnippetTitle.value.trim() || !newSnippetCmd.value.trim()) return
  const snip: CommandSnippet = {
    id: `snip-${Date.now()}`,
    title: newSnippetTitle.value.trim(),
    category: '自定义',
    command: newSnippetCmd.value.trim(),
    autoEnter: true,
    sortOrder: snippets.value.length + 1,
  }
  await api.saveSnippet(snip)
  snippets.value = await api.listSnippets()
  newSnippetTitle.value = ''
  newSnippetCmd.value = ''
}

async function removeSnippet(id: string) {
  await api.deleteSnippet(id)
  snippets.value = await api.listSnippets()
}

async function saveAppearance() {
  await store.saveTerminalAppearance(draftAppearance.value)
  appearanceOpen.value = false
  message.success('终端外观已保存')
}

watch(
  () => store.selectedSerial,
  (serial) => {
    if (serial && tabs.value.length === 0) {
      createTab()
    }
  },
  { immediate: true }
)

onMounted(async () => {
  snippets.value = await api.listSnippets()
})
</script>

<template>
  <div class="terminal-view">
    <div class="toolbar">
      <NSpace align="center" :size="8">
        <NButton size="small" type="primary" @click="createTab">新建终端</NButton>
        <NButton size="small" :disabled="!activeTabId" @click="clearCurrent">清屏</NButton>
        <NButton size="small" :disabled="!activeTabId" @click="reconnectCurrent">重连</NButton>
        <NButton
          size="small"
          @click="
            () => {
              draftAppearance = { ...store.terminalAppearance }
              appearanceOpen = true
            }
          "
        >
          外观设置
        </NButton>
        <NButton size="small" quaternary @click="snippetsOpen = !snippetsOpen">
          {{ snippetsOpen ? '收起快捷命令' : '展开快捷命令' }}
        </NButton>
      </NSpace>
    </div>

    <div class="main-split">
      <div class="tabs-area">
        <NEmpty v-if="tabs.length === 0" description="点击上方“新建终端”打开设备交互式 Shell" />
        <NTabs
          v-else
          v-model:value="activeTabId"
          type="card"
          closable
          size="small"
          style="height: 100%; display: flex; flex-direction: column"
          pane-style="flex: 1; min-height: 0; padding: 8px 0 0;"
          @close="closeTab"
        >
          <NTabPane v-for="t in tabs" :key="t.id" :name="t.id" :tab="t.title" display-directive="show">
            <div style="height: 100%">
              <TerminalPane
                :ref="(el) => setPaneRef(t.id, el)"
                :serial="t.serial"
                :appearance="store.terminalAppearance"
              />
            </div>
          </NTabPane>
        </NTabs>
      </div>

      <NCard v-if="snippetsOpen" title="快捷命令片段" size="small" class="snippets-panel">
        <NSpace vertical :size="10">
          <NList bordered size="small">
            <NListItem v-for="s in snippets" :key="s.id">
              <NSpace vertical :size="4">
                <NSpace justify="space-between" align="center">
                  <span style="font-weight: 600; font-size: 12px">{{ s.title }}</span>
                  <NTag size="tiny">{{ s.category }}</NTag>
                </NSpace>
                <code style="font-size: 11px; opacity: 0.75; word-break: break-all">{{ s.command }}</code>
                <NSpace :size="6">
                  <NButton size="tiny" type="primary" secondary @click="runSnippet(s, true)">执行</NButton>
                  <NButton size="tiny" secondary @click="runSnippet(s, false)">填入</NButton>
                  <NButton size="tiny" type="error" quaternary @click="removeSnippet(s.id)">删除</NButton>
                </NSpace>
              </NSpace>
            </NListItem>
          </NList>

          <NInput v-model:value="newSnippetTitle" size="small" placeholder="片段标题" />
          <NInput v-model:value="newSnippetCmd" size="small" placeholder="命令内容" />
          <NButton size="small" block @click="addSnippet">添加快捷命令</NButton>
        </NSpace>
      </NCard>
    </div>

    <!-- 终端外观设置弹窗 -->
    <NModal v-model:show="appearanceOpen" preset="card" title="终端外观与交互设置" style="width: 500px">
      <NForm label-placement="left" label-width="110">
        <NFormItem label="配色方案">
          <NSelect
            v-model:value="draftAppearance.theme"
            :options="[
              { label: '跟随系统主题 (自动)', value: 'auto' },
              { label: 'Light Clean', value: 'light-clean' },
              { label: 'One Dark', value: 'one-dark' },
              { label: 'Tokyo Night', value: 'tokyo-night' },
              { label: 'Catppuccin Mocha', value: 'catppuccin-mocha' },
            ]"
          />
        </NFormItem>
        <NFormItem label="字体族">
          <NInput v-model:value="draftAppearance.fontFamily" />
        </NFormItem>
        <NFormItem label="字号 (px)">
          <NSlider v-model:value="draftAppearance.fontSize" :min="11" :max="22" :step="1" />
        </NFormItem>
        <NFormItem label="行高">
          <NSlider v-model:value="draftAppearance.lineHeight" :min="1.0" :max="1.6" :step="0.05" />
        </NFormItem>
        <NFormItem label="光标样式">
          <NSelect
            v-model:value="draftAppearance.cursorStyle"
            :options="[
              { label: '竖线 (bar)', value: 'bar' },
              { label: '方块 (block)', value: 'block' },
              { label: '下划线 (underline)', value: 'underline' },
            ]"
          />
        </NFormItem>
        <NFormItem label="光标闪烁">
          <NSwitch v-model:value="draftAppearance.cursorBlink" />
        </NFormItem>
        <NFormItem label="滚回缓冲行数">
          <NInputNumber v-model:value="draftAppearance.scrollback" :min="500" :max="50000" :step="500" />
        </NFormItem>
        <NFormItem label="选中即自动复制">
          <NSwitch v-model:value="draftAppearance.copyOnSelect" />
        </NFormItem>
      </NForm>
      <template #footer>
        <NSpace justify="end">
          <NButton @click="appearanceOpen = false">取消</NButton>
          <NButton type="primary" @click="saveAppearance">应用并保存</NButton>
        </NSpace>
      </template>
    </NModal>
  </div>
</template>

<style scoped>
.terminal-view {
  display: flex;
  flex-direction: column;
  height: calc(100vh - 56px);
  gap: 10px;
}
.toolbar {
  flex-shrink: 0;
}
.main-split {
  flex: 1;
  min-height: 0;
  display: flex;
  gap: 12px;
}
.tabs-area {
  flex: 1;
  min-width: 0;
  min-height: 0;
}
.snippets-panel {
  width: 280px;
  flex-shrink: 0;
  overflow-y: auto;
}
</style>

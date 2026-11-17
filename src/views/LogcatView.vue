<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import {
  NButton,
  NCard,
  NCheckbox,
  NCheckboxGroup,
  NEmpty,
  NInput,
  NSelect,
  NSpace,
  NSwitch,
  NTag,
  NVirtualList,
  useMessage,
  type VirtualListInst,
} from 'naive-ui'
import { api } from '@/api/client'
import type { LogEntry } from '@/api/types'
import { useWorkbenchStore } from '@/stores/workbench'

const store = useWorkbenchStore()
const message = useMessage()

const running = ref(false)
const streamId = ref<string | null>(null)
const autoScroll = ref(true)
const buffers = ref<string[]>(['main', 'crash'])
const minLevel = ref<'V' | 'D' | 'I' | 'W' | 'E' | 'F'>('V')
const tagFilter = ref('')
const textFilter = ref('')
const useRegex = ref(false)
const pkgFilter = ref('')
const resolvedPids = ref<string[]>([])
const installedPackages = ref<string[]>([])

const rawEntries = ref<LogEntry[]>([])
const vlistRef = ref<VirtualListInst | null>(null)

const LEVEL_ORDER: Record<string, number> = { V: 0, D: 1, I: 2, W: 3, E: 4, F: 5 }
const LEVEL_COLORS: Record<string, string> = {
  V: '#8c8c8c',
  D: '#2080f0',
  I: '#18a058',
  W: '#f0a020',
  E: '#d03050',
  F: '#a8071a',
}

const filteredEntries = computed(() => {
  const minOrd = LEVEL_ORDER[minLevel.value] ?? 0
  const tagKw = tagFilter.value.trim().toLowerCase()
  const txt = textFilter.value.trim()
  let re: RegExp | null = null
  if (useRegex.value && txt) {
    try {
      re = new RegExp(txt, 'i')
    } catch {
      re = null
    }
  }
  const pids = resolvedPids.value

  return rawEntries.value.filter((e) => {
    if ((LEVEL_ORDER[e.level] ?? 2) < minOrd) return false
    if (pids.length > 0 && !pids.includes(e.pid)) return false
    if (tagKw && !e.tag.toLowerCase().includes(tagKw)) return false
    if (txt) {
      if (re) {
        if (!re.test(e.message) && !re.test(e.tag)) return false
      } else {
        const low = txt.toLowerCase()
        if (!e.message.toLowerCase().includes(low) && !e.tag.toLowerCase().includes(low)) {
          return false
        }
      }
    }
    return true
  })
})

async function startLogcat(clearFirst = false) {
  if (!store.selectedDevice) {
    message.warning('请先选择一台在线设备')
    return
  }
  await stopLogcat()
  try {
    const sid = await api.logcatStart(
      store.selectedDevice.serial,
      buffers.value,
      clearFirst,
      (batch) => {
        const max = store.logcatMaxLines
        const next = rawEntries.value.concat(batch)
        rawEntries.value = next.length > max ? next.slice(next.length - max) : next
        if (autoScroll.value) {
          void nextTick(() => {
            vlistRef.value?.scrollTo({ index: Math.max(0, filteredEntries.value.length - 1) })
          })
        }
      }
    )
    streamId.value = sid
    running.value = true
  } catch (e) {
    message.error(String(e))
  }
}

async function stopLogcat() {
  if (streamId.value) {
    await api.logcatStop(streamId.value).catch(() => {})
    streamId.value = null
  }
  running.value = false
}

async function clearView() {
  rawEntries.value = []
}

async function clearDeviceAndRestart() {
  rawEntries.value = []
  await startLogcat(true)
}

async function resolvePkgPid(pkg: string | null) {
  pkgFilter.value = pkg ?? ''
  if (!pkg || !store.selectedDevice) {
    resolvedPids.value = []
    return
  }
  try {
    const pids = await api.logcatResolvePkgPid(store.selectedDevice.serial, pkg)
    resolvedPids.value = pids
    if (pids.length === 0) {
      message.warning(`当前未检测到 ${pkg} 的运行进程 PID`)
    }
  } catch (e) {
    message.error(String(e))
  }
}

function exportLogFile() {
  const lines = filteredEntries.value.map((e) => e.raw).join('\n')
  const blob = new Blob([lines], { type: 'text/plain;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `logcat-${store.selectedSerial || 'device'}-${Date.now()}.log`
  a.click()
  URL.revokeObjectURL(url)
}

watch(
  () => store.selectedSerial,
  async (serial) => {
    await stopLogcat()
    rawEntries.value = []
    resolvedPids.value = []
    if (serial) {
      try {
        const apps = await api.listApps(serial, 'user')
        installedPackages.value = apps.map((a) => a.packageName)
      } catch {
        installedPackages.value = []
      }
    }
  },
  { immediate: true }
)

onBeforeUnmount(() => {
  void stopLogcat()
})
</script>

<template>
  <NCard title="实时 Logcat 日志流" size="small" class="logcat-card">
    <template #header-extra>
      <NSpace align="center" :size="8">
        <NTag size="small" :type="running ? 'success' : 'default'">
          {{ running ? `采集中 (${filteredEntries.length} / ${rawEntries.length})` : '已停止' }}
        </NTag>
        <NButton v-if="!running" size="small" type="primary" :disabled="!store.selectedDevice" @click="startLogcat(false)">
          开始采集
        </NButton>
        <NButton v-else size="small" type="warning" @click="stopLogcat">停止</NButton>
        <NButton size="small" @click="clearView">清空视图</NButton>
        <NButton size="small" :disabled="!store.selectedDevice" @click="clearDeviceAndRestart">
          清设备缓存 (logcat -c)
        </NButton>
        <NButton size="small" :disabled="filteredEntries.length === 0" @click="exportLogFile">导出 .log</NButton>
      </NSpace>
    </template>

    <NSpace vertical :size="10" style="height: 100%">
      <!-- 过滤工具栏 -->
      <NSpace align="center" :size="10" wrap>
        <NSelect
          v-model:value="minLevel"
          size="small"
          style="width: 110px"
          :options="[
            { label: 'Verbose+', value: 'V' },
            { label: 'Debug+', value: 'D' },
            { label: 'Info+', value: 'I' },
            { label: 'Warn+', value: 'W' },
            { label: 'Error+', value: 'E' },
            { label: 'Fatal', value: 'F' },
          ]"
        />
        <NSelect
          :value="pkgFilter || null"
          size="small"
          clearable
          filterable
          placeholder="按应用包名转 PID 过滤"
          style="width: 240px"
          :options="installedPackages.map((p) => ({ label: p, value: p }))"
          @update:value="resolvePkgPid"
        />
        <NInput v-model:value="tagFilter" size="small" placeholder="Tag 过滤" clearable style="width: 140px" />
        <NInput v-model:value="textFilter" size="small" placeholder="消息关键词 / 正则" clearable style="width: 200px" />
        <NCheckbox v-model:checked="useRegex" size="small">正则</NCheckbox>
        <NCheckboxGroup v-model:value="buffers" size="small">
          <NSpace :size="6">
            <NCheckbox value="main">main</NCheckbox>
            <NCheckbox value="system">system</NCheckbox>
            <NCheckbox value="crash">crash</NCheckbox>
            <NCheckbox value="events">events</NCheckbox>
          </NSpace>
        </NCheckboxGroup>
        <NSpace align="center" :size="4">
          <span style="font-size: 12px">自动滚底</span>
          <NSwitch v-model:value="autoScroll" size="small" />
        </NSpace>
      </NSpace>

      <!-- 虚拟滚动日志视图 -->
      <div class="log-container">
        <NEmpty v-if="filteredEntries.length === 0" description="暂无匹配日志，点击右上角“开始采集”" />
        <NVirtualList
          v-else
          ref="vlistRef"
          :items="filteredEntries"
          :item-size="24"
          key-field="id"
          style="height: 100%"
        >
          <template #default="{ item }">
            <div class="log-row" :style="{ color: LEVEL_COLORS[item.level] || 'inherit' }">
              <span class="col-ts">{{ item.timestamp }}</span>
              <span class="col-pid">{{ item.pid }}</span>
              <span class="col-lvl">{{ item.level }}</span>
              <span class="col-tag" :title="item.tag">{{ item.tag }}</span>
              <span class="col-msg">{{ item.message }}</span>
            </div>
          </template>
        </NVirtualList>
      </div>
    </NSpace>
  </NCard>
</template>

<style scoped>
.logcat-card {
  height: calc(100vh - 56px);
  display: flex;
  flex-direction: column;
}
.logcat-card :deep(.n-card__content) {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.log-container {
  flex: 1;
  min-height: 360px;
  height: calc(100vh - 180px);
  border: 1px solid rgba(128, 128, 128, 0.2);
  border-radius: 6px;
  padding: 6px 8px;
  font-family: 'JetBrains Mono', 'Fira Code', monospace;
  font-size: 12px;
}
.log-row {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 24px;
  line-height: 24px;
  white-space: nowrap;
}
.col-ts {
  width: 145px;
  flex-shrink: 0;
  opacity: 0.7;
}
.col-pid {
  width: 52px;
  flex-shrink: 0;
  opacity: 0.7;
}
.col-lvl {
  width: 18px;
  font-weight: 700;
  flex-shrink: 0;
}
.col-tag {
  width: 160px;
  flex-shrink: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  font-weight: 600;
}
.col-msg {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>

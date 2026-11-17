<script setup lang="ts">
import { computed, ref } from 'vue'
import { NEmpty, NIcon, NInput, NPopover, NSwitch, NTag } from 'naive-ui'
import {
  CheckmarkOutline,
  ChevronDownOutline,
  PhonePortraitOutline,
  SearchOutline,
  WifiOutline,
} from '@vicons/ionicons5'
import type { ActiveDeviceView } from '@/api/types'
import { useWorkbenchStore } from '@/stores/workbench'

const store = useWorkbenchStore()

const popoverOpen = ref(false)
const searchText = ref('')
const filterMode = ref<'all' | 'usb' | 'tcp' | 'ready'>('all')

const popoverStyle = computed(() =>
  store.isDark
    ? {
        backgroundColor: '#141418',
        border: '1px solid rgba(255, 255, 255, 0.12)',
        boxShadow: '0 12px 32px rgba(0, 0, 0, 0.65)',
      }
    : {
        boxShadow: '0 10px 28px rgba(0, 0, 0, 0.14)',
      }
)

function deviceTitle(d: ActiveDeviceView): string {
  return d.alias || d.marketName || d.model || d.serial
}

function deviceSubmeta(d: ActiveDeviceView): string {
  const parts: string[] = []
  if (d.alias && (d.marketName || d.model)) {
    parts.push(d.marketName || d.model || '')
  }
  if (d.androidVersion) {
    parts.push(`Android ${d.androidVersion}`)
  }
  return parts.join(' · ')
}

function stateTagType(state: string): 'success' | 'warning' | 'error' | 'default' {
  switch (state) {
    case 'device':
      return 'success'
    case 'unauthorized':
      return 'warning'
    case 'offline':
      return 'error'
    default:
      return 'default'
  }
}

function stateLabel(state: string): string {
  switch (state) {
    case 'device':
      return '在线'
    case 'unauthorized':
      return '未授权'
    case 'offline':
      return '离线'
    default:
      return state
  }
}

const filteredDevices = computed(() => {
  const kw = searchText.value.trim().toLowerCase()
  return store.activeDevices.filter((d) => {
    if (filterMode.value === 'usb' && d.transportType !== 'usb') return false
    if (filterMode.value === 'tcp' && d.transportType !== 'tcp') return false
    if (filterMode.value === 'ready' && d.state !== 'device') return false
    if (!kw) return true
    return (
      d.serial.toLowerCase().includes(kw) ||
      (d.alias?.toLowerCase().includes(kw) ?? false) ||
      (d.model?.toLowerCase().includes(kw) ?? false) ||
      (d.marketName?.toLowerCase().includes(kw) ?? false) ||
      (d.brand?.toLowerCase().includes(kw) ?? false)
    )
  })
})

function selectDevice(serial: string) {
  store.selectedSerial = serial
  popoverOpen.value = false
}

function handlePopoverUpdate(show: boolean) {
  popoverOpen.value = show
  if (!show) {
    searchText.value = ''
    filterMode.value = 'all'
  }
}
</script>

<template>
  <div class="switcher-root">
    <div class="section-label">
      <span>在线设备 ({{ store.activeDevices.length }})</span>
      <span class="auto-scan-control" :class="{ paused: !store.autoScanEnabled }">
        <span class="pulse-dot" :class="{ paused: !store.autoScanEnabled }" />
        <span>自动扫描</span>
        <NSwitch
          size="small"
          :value="store.autoScanEnabled"
          @update:value="store.setAutoScanEnabled"
        />
      </span>
    </div>

    <NPopover
      v-model:show="popoverOpen"
      trigger="click"
      placement="bottom-start"
      :width="348"
      :show-arrow="false"
      :style="popoverStyle"
      content-style="padding: 10px;"
      @update:show="handlePopoverUpdate"
    >
      <template #trigger>
        <div
          class="trigger-card"
          :class="{ empty: !store.selectedDevice, open: popoverOpen }"
          title="点击切换设备"
        >
          <template v-if="store.selectedDevice">
            <span
              class="color-dot"
              :style="{ backgroundColor: store.selectedDevice.colorTag || '#2080f0' }"
            />
            <NIcon size="15" class="transport-icon">
              <PhonePortraitOutline v-if="store.selectedDevice.transportType === 'usb'" />
              <WifiOutline v-else />
            </NIcon>
            <div class="trigger-main">
              <div class="trigger-title-row">
                <span class="trigger-title">{{ deviceTitle(store.selectedDevice) }}</span>
                <NTag
                  size="tiny"
                  :type="stateTagType(store.selectedDevice.state)"
                  :bordered="false"
                >
                  {{ stateLabel(store.selectedDevice.state) }}
                </NTag>
              </div>
              <div class="trigger-serial">{{ store.selectedDevice.serial }}</div>
            </div>
            <NIcon size="14" class="chevron-icon" :class="{ rotated: popoverOpen }">
              <ChevronDownOutline />
            </NIcon>
          </template>
          <template v-else>
            <span class="empty-trigger-text">暂无在线设备</span>
            <NIcon size="14" class="chevron-icon">
              <ChevronDownOutline />
            </NIcon>
          </template>
        </div>
      </template>

      <div class="popover-panel">
        <div class="popover-search-row">
          <NInput
            v-model:value="searchText"
            size="small"
            clearable
            placeholder="搜索名称 / 型号 / 序列号..."
          >
            <template #prefix>
              <NIcon size="14">
                <SearchOutline />
              </NIcon>
            </template>
          </NInput>
        </div>

        <div v-if="store.activeDevices.length > 1" class="filter-pills">
          <button
            v-for="item in [
              { key: 'all', label: `全部 (${store.activeDevices.length})` },
              { key: 'ready', label: '可用' },
              { key: 'usb', label: 'USB' },
              { key: 'tcp', label: 'Wi-Fi' },
            ]"
            :key="item.key"
            type="button"
            class="filter-pill"
            :class="{ active: filterMode === item.key }"
            @click="filterMode = item.key as typeof filterMode"
          >
            {{ item.label }}
          </button>
        </div>

        <div class="device-list-scroll">
          <div
            v-for="dev in filteredDevices"
            :key="dev.serial"
            class="compact-row"
            :class="{ active: store.selectedSerial === dev.serial }"
            @click="selectDevice(dev.serial)"
          >
            <span
              class="color-dot"
              :style="{ backgroundColor: dev.colorTag || '#2080f0' }"
            />
            <NIcon size="14" class="transport-icon">
              <PhonePortraitOutline v-if="dev.transportType === 'usb'" />
              <WifiOutline v-else />
            </NIcon>
            <div class="row-info">
              <div class="row-top">
                <span class="row-title">{{ deviceTitle(dev) }}</span>
                <span v-if="deviceSubmeta(dev)" class="row-meta">
                  {{ deviceSubmeta(dev) }}
                </span>
              </div>
              <div class="row-serial">{{ dev.serial }}</div>
            </div>
            <div class="row-right">
              <NTag size="tiny" :type="stateTagType(dev.state)" :bordered="false">
                {{ stateLabel(dev.state) }}
              </NTag>
              <NIcon
                v-if="store.selectedSerial === dev.serial"
                size="14"
                class="selected-check"
              >
                <CheckmarkOutline />
              </NIcon>
            </div>
          </div>

          <NEmpty
            v-if="filteredDevices.length === 0"
            size="small"
            description="没有匹配的在线设备"
            style="padding: 18px 0"
          />
        </div>
      </div>
    </NPopover>
  </div>
</template>

<style scoped>
.switcher-root {
  margin-bottom: 10px;
}
.section-label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 11px;
  color: rgba(160, 165, 175, 0.9);
  padding: 0 6px 6px;
}
.auto-scan-control {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 10.5px;
  color: #18a058;
}
.auto-scan-control.paused {
  color: rgba(160, 165, 175, 0.75);
}
.pulse-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: #18a058;
  animation: pulse 1.8s infinite ease-in-out;
}
.pulse-dot.paused {
  background-color: rgba(160, 165, 175, 0.55);
  animation: none;
}
@keyframes pulse {
  0%, 100% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.35;
    transform: scale(0.8);
  }
}
.trigger-card {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: 8px;
  cursor: pointer;
  border: 1px solid rgba(128, 128, 128, 0.28);
  background-color: rgba(128, 128, 128, 0.06);
  transition: background-color 0.15s, border-color 0.15s;
  user-select: none;
}
.trigger-card:hover,
.trigger-card.open {
  background-color: rgba(24, 160, 88, 0.08);
  border-color: rgba(24, 160, 88, 0.45);
}
.trigger-card.empty {
  justify-content: space-between;
  border-style: dashed;
  background-color: transparent;
}
.empty-trigger-text {
  font-size: 12px;
  opacity: 0.6;
}
.trigger-main {
  flex: 1;
  min-width: 0;
}
.trigger-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
}
.trigger-title {
  font-size: 13px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.trigger-serial {
  font-size: 11px;
  font-family: monospace;
  opacity: 0.62;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin-top: 1px;
}
.color-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.transport-icon {
  flex-shrink: 0;
  opacity: 0.82;
}
.chevron-icon {
  flex-shrink: 0;
  opacity: 0.65;
  transition: transform 0.18s ease;
}
.chevron-icon.rotated {
  transform: rotate(180deg);
}
.popover-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.filter-pills {
  display: flex;
  align-items: center;
  gap: 5px;
  flex-wrap: wrap;
}
.filter-pill {
  border: 1px solid rgba(128, 128, 128, 0.25);
  background: transparent;
  color: inherit;
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 999px;
  cursor: pointer;
  opacity: 0.78;
  transition: all 0.15s;
}
.filter-pill:hover {
  opacity: 1;
  border-color: rgba(24, 160, 88, 0.45);
}
.filter-pill.active {
  opacity: 1;
  color: #18a058;
  border-color: rgba(24, 160, 88, 0.6);
  background-color: rgba(24, 160, 88, 0.12);
}
.device-list-scroll {
  display: flex;
  flex-direction: column;
  gap: 3px;
  max-height: 320px;
  overflow-y: auto;
  padding-right: 2px;
}
.compact-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 6px;
  cursor: pointer;
  border: 1px solid transparent;
  transition: background-color 0.12s, border-color 0.12s;
}
.compact-row:hover {
  background-color: rgba(128, 128, 128, 0.1);
}
.compact-row.active {
  background-color: rgba(24, 160, 88, 0.12);
  border-color: rgba(24, 160, 88, 0.45);
}
.row-info {
  flex: 1;
  min-width: 0;
}
.row-top {
  display: flex;
  align-items: baseline;
  gap: 6px;
  min-width: 0;
}
.row-title {
  font-size: 12.5px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.row-meta {
  font-size: 11px;
  opacity: 0.58;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex-shrink: 1;
}
.row-serial {
  font-size: 11px;
  font-family: monospace;
  opacity: 0.58;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.row-right {
  display: flex;
  align-items: center;
  gap: 5px;
  flex-shrink: 0;
}
.selected-check {
  color: #18a058;
}
</style>

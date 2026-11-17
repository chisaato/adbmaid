<script setup lang="ts">
import { computed } from 'vue'
import { NIcon, NTag } from 'naive-ui'
import { PhonePortraitOutline, WifiOutline } from '@vicons/ionicons5'
import type { ActiveDeviceView } from '@/api/types'

const props = defineProps<{
  device: ActiveDeviceView
  compact?: boolean
}>()

const displayTitle = computed(() => {
  if (props.device.alias) return props.device.alias
  if (props.device.marketName) return props.device.marketName
  if (props.device.model) return props.device.model
  return props.device.serial
})

const stateType = computed(() => {
  switch (props.device.state) {
    case 'device':
      return 'success'
    case 'unauthorized':
      return 'warning'
    case 'offline':
      return 'error'
    default:
      return 'default'
  }
})
</script>

<template>
  <div class="device-badge">
    <span
      class="color-dot"
      :style="{ backgroundColor: device.colorTag || (device.state === 'device' ? '#18a058' : '#909399') }"
    />
    <NIcon :size="15" class="transport-icon">
      <WifiOutline v-if="device.transportType === 'tcp'" />
      <PhonePortraitOutline v-else />
    </NIcon>
    <div class="info">
      <div class="title-row">
        <span class="title">{{ displayTitle }}</span>
        <NTag v-if="!compact" size="tiny" :type="stateType" round>
          {{ device.state }}
        </NTag>
      </div>
      <div v-if="!compact" class="sub">
        {{ device.marketName || device.model || '未知型号' }} · {{ device.serial }}
      </div>
    </div>
  </div>
</template>

<style scoped>
.device-badge {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.color-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.transport-icon {
  flex-shrink: 0;
  opacity: 0.8;
}
.info {
  min-width: 0;
  flex: 1;
}
.title-row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.title {
  font-weight: 600;
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sub {
  font-size: 11px;
  opacity: 0.65;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>

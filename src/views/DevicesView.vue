<script setup lang="ts">
import { computed, h, ref, watch } from 'vue'
import {
  NButton,
  NCard,
  NColorPicker,
  NDataTable,
  NDescriptions,
  NDescriptionsItem,
  NDropdown,
  NEmpty,
  NForm,
  NFormItem,
  NGrid,
  NGi,
  NIcon,
  NInput,
  NModal,
  NSelect,
  NSpace,
  NStatistic,
  NSwitch,
  NTag,
  useDialog,
  useMessage,
  type DataTableColumns,
  type DropdownOption,
} from 'naive-ui'
import { ChevronDownOutline } from '@vicons/ionicons5'
import { api } from '@/api/client'
import type { DeviceEndpointRecord, DeviceRecord, DeviceTelemetry, ScrcpyProfile } from '@/api/types'
import { useWorkbenchStore } from '@/stores/workbench'

const store = useWorkbenchStore()
const message = useMessage()
const dialog = useDialog()

const telemetry = ref<DeviceTelemetry | null>(null)
const telemetryLoading = ref(false)

const editModalOpen = ref(false)
const editTarget = ref<DeviceRecord | null>(null)
const editAlias = ref('')
const editColor = ref('#18a058')
const editNotes = ref('')
const editAutoConnect = ref(false)
const editProfileId = ref<string | null>(null)
const profiles = ref<ScrcpyProfile[]>([])

const endpointsModalOpen = ref(false)
const endpointsList = ref<DeviceEndpointRecord[]>([])

const activeRecord = computed(() => {
  const uid = store.selectedDevice?.deviceUid
  if (!uid) return null
  return store.knownDevices.find((d) => d.deviceUid === uid) ?? null
})

async function loadTelemetry() {
  const dev = store.selectedDevice
  if (!dev || dev.state !== 'device') {
    telemetry.value = null
    return
  }
  telemetryLoading.value = true
  try {
    telemetry.value = await api.getDeviceTelemetry(dev.serial)
  } catch {
    telemetry.value = null
  } finally {
    telemetryLoading.value = false
  }
}

watch(
  () => store.selectedSerial,
  () => {
    void loadTelemetry()
  },
  { immediate: true }
)

async function openEditModal(rec: DeviceRecord) {
  editTarget.value = rec
  editAlias.value = rec.alias ?? ''
  editColor.value = rec.colorTag ?? '#18a058'
  editNotes.value = rec.notes ?? ''
  editAutoConnect.value = rec.autoConnect
  editProfileId.value = rec.defaultProfileId ?? null
  profiles.value = await api.scrcpyListProfiles()
  editModalOpen.value = true
}

async function saveMeta() {
  if (!editTarget.value) return
  try {
    await api.updateDeviceMeta({
      deviceUid: editTarget.value.deviceUid,
      alias: editAlias.value.trim() || null,
      colorTag: editColor.value,
      notes: editNotes.value.trim() || null,
      autoConnect: editAutoConnect.value,
      defaultProfileId: editProfileId.value,
    })
    await store.refreshKnownDevices()
    await store.refreshActiveDevices()
    editModalOpen.value = false
    message.success('设备信息已更新')
  } catch (e) {
    message.error(String(e))
  }
}

async function openEndpoints(rec: DeviceRecord) {
  endpointsList.value = await api.listDeviceEndpoints(rec.deviceUid)
  endpointsModalOpen.value = true
}

async function handleEnableTcpip() {
  if (!store.selectedDevice) return
  try {
    const res = await api.enableTcpip(store.selectedDevice.serial, 5555)
    message.success(res || '已开启 tcpip 5555')
  } catch (e) {
    message.error(String(e))
  }
}

async function handleDisconnect() {
  const dev = store.selectedDevice
  if (!dev || dev.transportType !== 'tcp') return
  const [host, portStr] = dev.serial.split(':')
  const port = Number(portStr)
  if (!host || !port) return
  try {
    await api.disconnectTcp(host, port)
    await store.refreshActiveDevices()
    message.success(`已断开 ${dev.serial}`)
  } catch (e) {
    message.error(String(e))
  }
}

const rebootOptions: DropdownOption[] = [
  { label: '正常重启 (System)', key: 'system' },
  { label: '重启到 Bootloader (Fastboot)', key: 'bootloader' },
  { label: '重启到 Recovery', key: 'recovery' },
  { label: '重启到 Sideload', key: 'sideload' },
]

function onSelectRebootMode(key: string) {
  if (!store.selectedDevice) return
  const mode = key as 'system' | 'bootloader' | 'recovery' | 'sideload'
  const labelMap: Record<typeof mode, string> = {
    system: '正常重启 (System)',
    bootloader: 'Bootloader (Fastboot)',
    recovery: 'Recovery',
    sideload: 'Sideload',
  }
  const targetName = store.selectedDevice.alias || store.selectedDevice.serial
  dialog.warning({
    title: '确认重启设备',
    content: `确定将 ${targetName} ${mode === 'system' ? '正常重启' : `重启到 ${labelMap[mode]}`} 吗？`,
    positiveText: '确认重启',
    negativeText: '取消',
    onPositiveClick: () => handleReboot(mode),
  })
}

async function handleReboot(mode: 'system' | 'bootloader' | 'recovery' | 'sideload') {
  if (!store.selectedDevice) return
  try {
    await api.rebootDevice(store.selectedDevice.serial, mode)
    message.info(`已发送重启指令 (${mode})`)
  } catch (e) {
    message.error(String(e))
  }
}

const knownColumns: DataTableColumns<DeviceRecord> = [
  {
    title: '设备名称 / 备注',
    key: 'alias',
    render: (row) =>
      h(NSpace, { align: 'center', size: 8 }, () => [
        h('span', {
          style: {
            width: '10px',
            height: '10px',
            borderRadius: '50%',
            display: 'inline-block',
            backgroundColor: row.colorTag || '#909399',
          },
        }),
        h('strong', row.alias || row.marketName || row.model || '未命名设备'),
      ]),
  },
  {
    title: '品牌 / 型号',
    key: 'model',
    render: (row) => `${row.brand || ''} ${row.marketName || row.model || '-'}`.trim(),
  },
  {
    title: '系统',
    key: 'androidVersion',
    width: 110,
    render: (row) => (row.androidVersion ? `Android ${row.androidVersion}` : '-'),
  },
  {
    title: '硬件序列号',
    key: 'hardwareSerial',
    render: (row) => h('code', row.hardwareSerial || '-'),
  },
  {
    title: '自动连接',
    key: 'autoConnect',
    width: 90,
    render: (row) =>
      h(NTag, { size: 'small', type: row.autoConnect ? 'success' : 'default' }, () =>
        row.autoConnect ? '开启' : '关闭'
      ),
  },
  {
    title: '操作',
    key: 'actions',
    width: 190,
    render: (row) =>
      h(NSpace, { size: 6 }, () => [
        h(NButton, { size: 'tiny', onClick: () => openEditModal(row) }, () => '编辑'),
        h(NButton, { size: 'tiny', quaternary: true, onClick: () => openEndpoints(row) }, () => '端点历史'),
      ]),
  },
]
</script>

<template>
  <NSpace vertical :size="16">
    <!-- 当前选中设备概览 -->
    <NCard title="当前设备概览" size="small">
      <template #header-extra>
        <NSpace v-if="store.selectedDevice" :size="8">
          <NButton size="small" @click="loadTelemetry" :loading="telemetryLoading">刷新状态</NButton>
          <NButton
            v-if="store.selectedDevice.transportType === 'usb'"
            size="small"
            @click="handleEnableTcpip"
          >
            开启 tcpip 5555
          </NButton>
          <NButton
            v-else
            size="small"
            type="warning"
            ghost
            @click="handleDisconnect"
          >
            断开无线连接
          </NButton>
          <NButton size="small" type="primary" @click="store.activeNav = 'scrcpy'">去投屏</NButton>
          <NDropdown trigger="click" :options="rebootOptions" @select="onSelectRebootMode">
            <NButton size="small" type="error" ghost icon-placement="right">
              重启设备
              <template #icon>
                <NIcon size="13">
                  <ChevronDownOutline />
                </NIcon>
              </template>
            </NButton>
          </NDropdown>
        </NSpace>
      </template>

      <NEmpty
        v-if="!store.selectedDevice"
        description="暂未选中在线设备，请通过 USB 连接或前往无线发现连接"
      />
      <template v-else>
        <NGrid :cols="4" :x-gap="12" :y-gap="12" style="margin-bottom: 14px">
          <NGi>
            <NCard size="small" embedded>
              <NStatistic
                label="电量"
                :value="telemetry?.batteryLevel !== null && telemetry?.batteryLevel !== undefined ? `${telemetry.batteryLevel}%` : '—'"
              >
                <template #suffix>
                  <NTag v-if="telemetry?.batteryCharging" size="tiny" type="success">充电中</NTag>
                </template>
              </NStatistic>
            </NCard>
          </NGi>
          <NGi>
            <NCard size="small" embedded>
              <NStatistic label="屏幕分辨率" :value="telemetry?.screenSize || '—'" />
            </NCard>
          </NGi>
          <NGi>
            <NCard size="small" embedded>
              <NStatistic label="无线 IP" :value="telemetry?.ipAddress || '—'" />
            </NCard>
          </NGi>
          <NGi>
            <NCard size="small" embedded>
              <NStatistic label="存储空间">
                <span class="compact-stat-value">{{ telemetry?.storageSummary || '—' }}</span>
              </NStatistic>
            </NCard>
          </NGi>
        </NGrid>

        <NDescriptions
          v-if="activeRecord"
          bordered
          size="small"
          :column="1"
          label-placement="left"
          :label-style="{ width: '150px' }"
        >
          <NDescriptionsItem label="备注名">{{ activeRecord.alias || '未设置' }}</NDescriptionsItem>
          <NDescriptionsItem label="市场型号">
            {{ activeRecord.brand }} {{ activeRecord.marketName || activeRecord.model }}
          </NDescriptionsItem>
          <NDescriptionsItem label="Android 版本">
            Android {{ activeRecord.androidVersion }} (SDK {{ activeRecord.sdkInt }})
          </NDescriptionsItem>
          <NDescriptionsItem label="硬件序列号">
            <code>{{ activeRecord.hardwareSerial || '-' }}</code>
          </NDescriptionsItem>
          <NDescriptionsItem label="CPU ABI">{{ activeRecord.abi || '-' }}</NDescriptionsItem>
          <NDescriptionsItem label="Device Code">{{ activeRecord.deviceCode || '-' }}</NDescriptionsItem>
          <NDescriptionsItem label="Build Fingerprint">
            <code style="font-size: 12px; word-break: break-all">{{ activeRecord.buildFingerprint || '-' }}</code>
          </NDescriptionsItem>
        </NDescriptions>
      </template>
    </NCard>

    <!-- 已知设备库 -->
    <NCard title="已记录设备指纹库 (SQLite)" size="small">
      <template #header-extra>
        <NButton size="small" @click="store.refreshKnownDevices()">刷新</NButton>
      </template>
      <NDataTable
        :columns="knownColumns"
        :data="store.knownDevices"
        :row-key="(r: DeviceRecord) => r.deviceUid"
        size="small"
      />
    </NCard>

    <!-- 编辑备注弹窗 -->
    <NModal v-model:show="editModalOpen" preset="card" title="编辑设备元信息" style="width: 480px">
      <NForm label-placement="left" label-width="100">
        <NFormItem label="备注名称">
          <NInput v-model:value="editAlias" placeholder="如：主力机 / 测试机 A" />
        </NFormItem>
        <NFormItem label="标识颜色">
          <NColorPicker v-model:value="editColor" :modes="['hex']" :show-alpha="false" />
        </NFormItem>
        <NFormItem label="默认投屏预设">
          <NSelect
            v-model:value="editProfileId"
            clearable
            :options="profiles.map((p) => ({ label: p.name, value: p.id }))"
            placeholder="选择默认投屏预设"
          />
        </NFormItem>
        <NFormItem label="mDNS 自动连接">
          <NSwitch v-model:value="editAutoConnect" />
        </NFormItem>
        <NFormItem label="设备备注">
          <NInput v-model:value="editNotes" type="textarea" :rows="3" />
        </NFormItem>
      </NForm>
      <template #footer>
        <NSpace justify="end">
          <NButton @click="editModalOpen = false">取消</NButton>
          <NButton type="primary" @click="saveMeta">保存</NButton>
        </NSpace>
      </template>
    </NModal>

    <!-- 端点历史弹窗 -->
    <NModal v-model:show="endpointsModalOpen" preset="card" title="历史连接端点" style="width: 680px">
      <NDataTable
        size="small"
        :data="endpointsList"
        :columns="[
          { title: '传输类型', key: 'transportType', width: 120 },
          { title: 'ADB Serial', key: 'adbSerial' },
          { title: 'mDNS 服务名', key: 'mdnsServiceName' },
          {
            title: '最近 IP:Port',
            key: 'ip',
            render: (r: DeviceEndpointRecord) => (r.lastIp ? `${r.lastIp}:${r.lastPort || ''}` : '-'),
          },
        ]"
      />
    </NModal>
  </NSpace>
</template>

<style scoped>
.compact-stat-value {
  display: inline-block;
  font-size: 15px;
  font-weight: 500;
  line-height: 28px;
  white-space: nowrap;
}
</style>

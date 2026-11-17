<script setup lang="ts">
import { h, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  NButton,
  NCard,
  NDataTable,
  NForm,
  NFormItem,
  NInput,
  NInputNumber,
  NModal,
  NSpace,
  NSwitch,
  NTag,
  useMessage,
  type DataTableColumns,
} from 'naive-ui'
import { api } from '@/api/client'
import type { MdnsDiscoveredItem } from '@/api/types'
import { useWorkbenchStore } from '@/stores/workbench'

const store = useWorkbenchStore()
const message = useMessage()

const items = ref<MdnsDiscoveredItem[]>([])
const scanning = ref(false)
const watching = ref(true)

const manualHost = ref('192.168.1.')
const manualPort = ref(5555)
const connecting = ref(false)

const pairModalOpen = ref(false)
const pairHost = ref('')
const pairPort = ref(37000)
const pairCode = ref('')
const pairing = ref(false)

let unlisten: (() => void) | null = null

async function scanOnce() {
  scanning.value = true
  try {
    items.value = await api.mdnsScanOnce()
  } catch (e) {
    message.error(String(e))
  } finally {
    scanning.value = false
  }
}

async function toggleWatch(val: boolean) {
  watching.value = val
  await api.mdnsWatch(val)
}

async function handleManualConnect() {
  if (!manualHost.value.trim()) return
  connecting.value = true
  try {
    await api.connectTcp(manualHost.value.trim(), manualPort.value)
    await store.refreshActiveDevices()
    await store.refreshKnownDevices()
    message.success(`已连接 ${manualHost.value}:${manualPort.value}`)
  } catch (e) {
    message.error(String(e))
  } finally {
    connecting.value = false
  }
}

async function connectDiscovered(item: MdnsDiscoveredItem) {
  try {
    await api.connectTcp(item.ip, item.port)
    await store.refreshActiveDevices()
    await store.refreshKnownDevices()
    await scanOnce()
    message.success(`已连接 ${item.matchedAlias || item.ip}`)
  } catch (e) {
    message.error(String(e))
  }
}

function openPairModal(item?: MdnsDiscoveredItem) {
  if (item) {
    pairHost.value = item.ip
    pairPort.value = item.port
  } else {
    pairHost.value = ''
    pairPort.value = 37000
  }
  pairCode.value = ''
  pairModalOpen.value = true
}

async function submitPair() {
  if (!pairHost.value.trim() || !pairCode.value.trim()) {
    message.warning('请填写 IP 地址与 6 位配对码')
    return
  }
  pairing.value = true
  try {
    await api.pairWireless(pairHost.value.trim(), pairPort.value, pairCode.value.trim())
    pairModalOpen.value = false
    message.success('无线配对成功，可在雷达列表中直接连接')
    await scanOnce()
  } catch (e) {
    message.error(String(e))
  } finally {
    pairing.value = false
  }
}

const confidenceLabel = (c: MdnsDiscoveredItem['confidence']) => {
  switch (c) {
    case 'exact':
      return { text: '已识别（精确）', type: 'success' as const }
    case 'history':
      return { text: '已识别（历史服务）', type: 'info' as const }
    case 'suspect':
      return { text: '疑似已知设备', type: 'warning' as const }
    default:
      return { text: '新设备', type: 'default' as const }
  }
}

const columns: DataTableColumns<MdnsDiscoveredItem> = [
  {
    title: '设备身份识别',
    key: 'identity',
    render: (row) => {
      const badge = confidenceLabel(row.confidence)
      return h(NSpace, { vertical: true, size: 2 }, () => [
        h(NSpace, { align: 'center', size: 6 }, () => [
          row.matchedColorTag
            ? h('span', {
                style: {
                  width: '8px',
                  height: '8px',
                  borderRadius: '50%',
                  backgroundColor: row.matchedColorTag,
                  display: 'inline-block',
                },
              })
            : null,
          h('strong', row.matchedAlias || row.matchedModel || row.serviceName),
          h(NTag, { size: 'tiny', type: badge.type }, () => badge.text),
        ]),
        h('span', { style: 'font-size: 12px; opacity: 0.65' }, row.serviceName),
      ])
    },
  },
  {
    title: '地址',
    key: 'addr',
    width: 170,
    render: (row) => h('code', `${row.ip}:${row.port}`),
  },
  {
    title: '类型 / 通道',
    key: 'regType',
    width: 210,
    render: (row) =>
      h(NSpace, { size: 4 }, () => [
        h(NTag, { size: 'tiny' }, () =>
          row.regType.includes('pairing') ? '无线配对' : '无线调试'
        ),
        h(NTag, { size: 'tiny', bordered: false }, () => row.source),
      ]),
  },
  {
    title: '操作',
    key: 'actions',
    width: 130,
    render: (row) => {
      if (row.status === 'connected') {
        return h(NTag, { type: 'success', size: 'small' }, () => '已连接')
      }
      if (row.status === 'ready_pair') {
        return h(
          NButton,
          { size: 'small', type: 'warning', onClick: () => openPairModal(row) },
          () => '输入配对码'
        )
      }
      return h(
        NButton,
        { size: 'small', type: 'primary', onClick: () => connectDiscovered(row) },
        () => '一键连接'
      )
    },
  },
]

onMounted(async () => {
  unlisten = await api.onMdnsUpdated((list) => {
    items.value = list
  })
  await api.mdnsWatch(true)
  await scanOnce()
})

onBeforeUnmount(() => {
  unlisten?.()
  void api.mdnsWatch(false)
})
</script>

<template>
  <NSpace vertical :size="16">
    <!-- 手动连接与配对 -->
    <NCard title="无线连接与配对" size="small">
      <NSpace align="center" :size="10" wrap>
        <NInput v-model:value="manualHost" placeholder="设备 IP，如 192.168.1.50" style="width: 200px" />
        <NInputNumber v-model:value="manualPort" :min="1" :max="65535" style="width: 120px" />
        <NButton type="primary" :loading="connecting" @click="handleManualConnect">TCP 连接</NButton>
        <NButton @click="openPairModal()">Android 11+ 无线配对...</NButton>
      </NSpace>
    </NCard>

    <!-- mDNS 雷达 -->
    <NCard title="局域网 mDNS 雷达（ADB + mdns-sd 双路融合）" size="small">
      <template #header-extra>
        <NSpace align="center" :size="12">
          <NSpace align="center" :size="6">
            <span style="font-size: 12px">持续监听</span>
            <NSwitch :value="watching" size="small" @update:value="toggleWatch" />
          </NSpace>
          <NButton size="small" :loading="scanning" @click="scanOnce">立即扫描</NButton>
        </NSpace>
      </template>

      <NDataTable
        :columns="columns"
        :data="items"
        :row-key="(r: MdnsDiscoveredItem) => r.id"
        size="small"
      />
    </NCard>

    <!-- 配对弹窗 -->
    <NModal v-model:show="pairModalOpen" preset="card" title="Android 11+ 无线调试配对" style="width: 420px">
      <NForm label-placement="left" label-width="90">
        <NFormItem label="IP 地址">
          <NInput v-model:value="pairHost" placeholder="192.168.1.x" />
        </NFormItem>
        <NFormItem label="配对端口">
          <NInputNumber v-model:value="pairPort" :min="1" :max="65535" style="width: 100%" />
        </NFormItem>
        <NFormItem label="6 位配对码">
          <NInput v-model:value="pairCode" placeholder="手机“使用配对码配对设备”显示的数字" />
        </NFormItem>
      </NForm>
      <template #footer>
        <NSpace justify="end">
          <NButton @click="pairModalOpen = false">取消</NButton>
          <NButton type="primary" :loading="pairing" @click="submitPair">执行配对</NButton>
        </NSpace>
      </template>
    </NModal>
  </NSpace>
</template>

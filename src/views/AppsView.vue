<script setup lang="ts">
import { computed, h, ref, watch } from 'vue'
import {
  NButton,
  NCard,
  NCheckbox,
  NDataTable,
  NDrawer,
  NDrawerContent,
  NEmpty,
  NInput,
  NList,
  NListItem,
  NPopconfirm,
  NRadioButton,
  NRadioGroup,
  NSpace,
  NTag,
  useMessage,
  type DataTableColumns,
} from 'naive-ui'
import { api } from '@/api/client'
import type { AppItem, InstallFlags, InstallProgress } from '@/api/types'
import { useWorkbenchStore } from '@/stores/workbench'

const store = useWorkbenchStore()
const message = useMessage()

const scope = ref<'user' | 'system' | 'all'>('user')
const keyword = ref('')
const loading = ref(false)
const apps = ref<AppItem[]>([])

const installDrawerOpen = ref(false)
const apkPathsInput = ref('')
const installFlags = ref<InstallFlags>({
  replace: true,
  downgrade: false,
  grantPermissions: true,
  allowTest: false,
})
const installing = ref(false)
const installLogs = ref<InstallProgress[]>([])

const filteredApps = computed(() => {
  const kw = keyword.value.trim().toLowerCase()
  if (!kw) return apps.value
  return apps.value.filter(
    (a) =>
      a.packageName.toLowerCase().includes(kw) || a.apkPath.toLowerCase().includes(kw)
  )
})

async function fetchApps() {
  const dev = store.selectedDevice
  if (!dev || dev.state !== 'device') {
    apps.value = []
    return
  }
  loading.value = true
  try {
    apps.value = await api.listApps(dev.serial, scope.value)
  } catch (e) {
    message.error(String(e))
  } finally {
    loading.value = false
  }
}

watch(
  () => [store.selectedSerial, scope.value, store.apkInstallSeq],
  () => {
    void fetchApps()
  },
  { immediate: true }
)

async function handleAction(pkg: string, action: 'launch' | 'stop' | 'clear' | 'uninstall' | 'pull') {
  if (!store.selectedDevice) return
  try {
    let destPath: string | undefined
    if (action === 'pull') {
      const input = window.prompt('请输入导出到本地的完整文件路径:', `/tmp/${pkg}.apk`)
      if (!input) return
      destPath = input
    }
    const res = await api.appAction(store.selectedDevice.serial, pkg, action, destPath)
    message.success(res || '操作成功')
    if (action === 'uninstall') {
      await fetchApps()
    }
  } catch (e) {
    message.error(String(e))
  }
}

async function handleInstall() {
  if (!store.selectedDevice) return
  const paths = apkPathsInput.value
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean)
  if (!paths.length) {
    message.warning('请输入至少一个 .apk 文件路径')
    return
  }
  installing.value = true
  installLogs.value = []
  try {
    await api.installApks(store.selectedDevice.serial, paths, installFlags.value, (p) => {
      const idx = installLogs.value.findIndex((x) => x.apkPath === p.apkPath)
      if (idx >= 0) installLogs.value[idx] = p
      else installLogs.value.push(p)
    })
    await fetchApps()
    message.success('安装任务处理完成')
  } catch (e) {
    message.error(String(e))
  } finally {
    installing.value = false
  }
}

const columns: DataTableColumns<AppItem> = [
  {
    title: '包名',
    key: 'packageName',
    render: (row) =>
      h(NSpace, { align: 'center', size: 6 }, () => [
        h('strong', row.packageName),
        row.isSystem ? h(NTag, { size: 'tiny', type: 'default' }, () => '系统') : null,
      ]),
  },
  {
    title: 'APK 路径',
    key: 'apkPath',
    ellipsis: { tooltip: true },
    render: (row) => h('code', { style: 'font-size: 12px; opacity: 0.75' }, row.apkPath),
  },
  {
    title: '操作',
    key: 'actions',
    width: 310,
    render: (row) =>
      h(NSpace, { size: 4 }, () => [
        h(NButton, { size: 'tiny', type: 'primary', quaternary: true, onClick: () => handleAction(row.packageName, 'launch') }, () => '启动'),
        h(NButton, { size: 'tiny', quaternary: true, onClick: () => handleAction(row.packageName, 'stop') }, () => '强停'),
        h(NButton, { size: 'tiny', quaternary: true, onClick: () => handleAction(row.packageName, 'clear') }, () => '清数据'),
        h(NButton, { size: 'tiny', quaternary: true, onClick: () => handleAction(row.packageName, 'pull') }, () => '导出'),
        h(
          NPopconfirm,
          { onPositiveClick: () => handleAction(row.packageName, 'uninstall') },
          {
            trigger: () => h(NButton, { size: 'tiny', type: 'error', quaternary: true }, () => '卸载'),
            default: () => `确认卸载 ${row.packageName} ？`,
          }
        ),
      ]),
  },
]
</script>

<template>
  <NCard title="应用管理 (App Manager)" size="small">
    <template #header-extra>
      <NSpace align="center" :size="10">
        <NRadioGroup v-model:value="scope" size="small">
          <NRadioButton value="user">第三方</NRadioButton>
          <NRadioButton value="system">系统</NRadioButton>
          <NRadioButton value="all">全部</NRadioButton>
        </NRadioGroup>
        <NInput v-model:value="keyword" size="small" placeholder="搜索包名或路径..." clearable style="width: 220px" />
        <NButton size="small" @click="fetchApps" :loading="loading">刷新</NButton>
        <NButton size="small" type="primary" :disabled="!store.selectedDevice" @click="installDrawerOpen = true">
          安装 APK
        </NButton>
      </NSpace>
    </template>

    <NEmpty v-if="!store.selectedDevice" description="请先在左侧选择一台在线设备" />
    <NDataTable
      v-else
      :columns="columns"
      :data="filteredApps"
      :loading="loading"
      :row-key="(r: AppItem) => r.packageName"
      :pagination="{ pageSize: 15 }"
      size="small"
    />

    <NDrawer v-model:show="installDrawerOpen" :width="480">
      <NDrawerContent title="批量安装 APK">
        <NSpace vertical :size="12">
          <NInput
            v-model:value="apkPathsInput"
            type="textarea"
            :rows="5"
            placeholder="每行输入一个本地 .apk 文件绝对路径（支持批量安装）"
          />
          <NSpace wrap>
            <NCheckbox v-model:checked="installFlags.replace">-r 覆盖安装</NCheckbox>
            <NCheckbox v-model:checked="installFlags.downgrade">-d 允许降级</NCheckbox>
            <NCheckbox v-model:checked="installFlags.grantPermissions">-g 自动授权</NCheckbox>
            <NCheckbox v-model:checked="installFlags.allowTest">-t 允许测试包</NCheckbox>
          </NSpace>
          <NButton type="primary" block :loading="installing" @click="handleInstall">开始安装</NButton>

          <NList v-if="installLogs.length" bordered size="small">
            <NListItem v-for="item in installLogs" :key="item.apkPath">
              <NSpace vertical :size="2">
                <NSpace justify="space-between">
                  <span style="font-size: 12px; font-weight: 600">{{ item.apkPath }}</span>
                  <NTag
                    size="tiny"
                    :type="item.status === 'success' ? 'success' : item.status === 'error' ? 'error' : 'info'"
                  >
                    {{ item.status }}
                  </NTag>
                </NSpace>
                <span style="font-size: 12px; opacity: 0.7">{{ item.message }}</span>
              </NSpace>
            </NListItem>
          </NList>
        </NSpace>
      </NDrawerContent>
    </NDrawer>
  </NCard>
</template>

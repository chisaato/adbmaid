<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import {
  NButton,
  NCard,
  NForm,
  NFormItem,
  NGrid,
  NGi,
  NInput,
  NInputNumber,
  NList,
  NListItem,
  NSelect,
  NSpace,
  NSwitch,
  NTag,
  useMessage,
} from 'naive-ui'
import { api } from '@/api/client'
import type { ScrcpyInstance, ScrcpyOptions, ScrcpyProfile } from '@/api/types'
import { useWorkbenchStore } from '@/stores/workbench'

const store = useWorkbenchStore()
const message = useMessage()

const profiles = ref<ScrcpyProfile[]>([])
const selectedProfileId = ref<string>('builtin-balanced')
const runningInstances = ref<ScrcpyInstance[]>([])
const newProfileName = ref('')
const starting = ref(false)

const options = ref<ScrcpyOptions>({
  maxSize: 1920,
  videoBitRate: '8M',
  maxFps: 60,
  videoCodec: 'h264',
  turnScreenOff: false,
  stayAwake: true,
  alwaysOnTop: false,
  showTouches: false,
  readOnly: false,
  keyboardMode: 'sdk',
  mouseMode: 'sdk',
  noAudio: false,
  audioCodec: 'opus',
  newDisplay: null,
  startApp: null,
  windowTitle: null,
  extraArgs: null,
})

let unlistenExit: (() => void) | null = null

async function loadData() {
  profiles.value = await api.scrcpyListProfiles()
  runningInstances.value = await api.scrcpyListRunning()
  applyProfile(selectedProfileId.value)
}

function applyProfile(id: string) {
  selectedProfileId.value = id
  const found = profiles.value.find((p) => p.id === id)
  if (found) {
    options.value = { ...found.options }
  }
}

async function saveAsProfile() {
  if (!newProfileName.value.trim()) {
    message.warning('请输入预设名称')
    return
  }
  const profile: ScrcpyProfile = {
    id: `custom-${Date.now()}`,
    name: newProfileName.value.trim(),
    isBuiltin: false,
    options: { ...options.value },
    updatedAt: Date.now(),
  }
  await api.scrcpySaveProfile(profile)
  newProfileName.value = ''
  profiles.value = await api.scrcpyListProfiles()
  selectedProfileId.value = profile.id
  message.success(`已保存预设 "${profile.name}"`)
}

async function startMirror() {
  if (!store.selectedDevice) {
    message.warning('请先在左侧选择一台在线设备')
    return
  }
  starting.value = true
  try {
    const profileName = profiles.value.find((p) => p.id === selectedProfileId.value)?.name ?? null
    const inst = await api.scrcpyStart(store.selectedDevice.serial, profileName, options.value)
    runningInstances.value.push(inst)
    message.success(`已拉起原生 scrcpy 投屏窗口 (${store.selectedDevice.serial})`)
  } catch (e) {
    message.error(String(e))
  } finally {
    starting.value = false
  }
}

async function stopInstance(instanceId: string) {
  await api.scrcpyStop(instanceId)
  runningInstances.value = runningInstances.value.filter((i) => i.instanceId !== instanceId)
}

onMounted(async () => {
  await loadData()
  unlistenExit = await api.onScrcpyExited((ev) => {
    runningInstances.value = runningInstances.value.filter((i) => i.instanceId !== ev.instanceId)
    if (ev.code !== 0 && ev.stderrTail) {
      message.error(`投屏进程异常退出 (code=${ev.code}): ${ev.stderrTail}`)
    } else {
      message.info(`投屏窗口已关闭 (${ev.serial})`)
    }
  })
})

onBeforeUnmount(() => {
  unlistenExit?.()
})
</script>

<template>
  <NSpace vertical :size="16">
    <!-- 预设与启动栏 -->
    <NCard title="scrcpy 原生投屏托管（环境变量强隔离）" size="small">
      <template #header-extra>
        <NSpace align="center" :size="8">
          <NSelect
            :value="selectedProfileId"
            size="small"
            style="width: 190px"
            :options="profiles.map((p) => ({ label: p.isBuiltin ? `${p.name} (内置)` : p.name, value: p.id }))"
            @update:value="applyProfile"
          />
          <NInput v-model:value="newProfileName" size="small" placeholder="另存为新预设..." style="width: 150px" />
          <NButton size="small" @click="saveAsProfile">保存预设</NButton>
          <NButton
            size="small"
            type="primary"
            :loading="starting"
            :disabled="!store.selectedDevice"
            @click="startMirror"
          >
            启动投屏窗口
          </NButton>
        </NSpace>
      </template>

      <NGrid :cols="2" :x-gap="16" :y-gap="12">
        <NGi>
          <NCard size="small" title="视频编码与分辨率" embedded>
            <NForm label-placement="left" label-width="110" size="small">
              <NFormItem label="最大分辨率">
                <NSelect
                  v-model:value="options.maxSize"
                  clearable
                  placeholder="原始分辨率"
                  :options="[
                    { label: '1920 px', value: 1920 },
                    { label: '1600 px', value: 1600 },
                    { label: '1280 px', value: 1280 },
                    { label: '1024 px', value: 1024 },
                  ]"
                />
              </NFormItem>
              <NFormItem label="视频码率">
                <NSelect
                  v-model:value="options.videoBitRate"
                  :options="[
                    { label: '4 Mbps', value: '4M' },
                    { label: '8 Mbps (推荐)', value: '8M' },
                    { label: '12 Mbps', value: '12M' },
                    { label: '16 Mbps', value: '16M' },
                  ]"
                />
              </NFormItem>
              <NFormItem label="最大帧率">
                <NInputNumber v-model:value="options.maxFps" :min="15" :max="240" style="width: 100%" />
              </NFormItem>
              <NFormItem label="视频编码器">
                <NSelect
                  v-model:value="options.videoCodec"
                  :options="[
                    { label: 'H.264 (兼容性最佳)', value: 'h264' },
                    { label: 'H.265 (HEVC)', value: 'h265' },
                    { label: 'AV1', value: 'av1' },
                  ]"
                />
              </NFormItem>
            </NForm>
          </NCard>
        </NGi>

        <NGi>
          <NCard size="small" title="显示、控制与音频" embedded>
            <NForm label-placement="left" label-width="120" size="small">
              <NFormItem label="投屏时熄屏 (-S)">
                <NSwitch v-model:value="options.turnScreenOff" />
              </NFormItem>
              <NFormItem label="保持唤醒 (-w)">
                <NSwitch v-model:value="options.stayAwake" />
              </NFormItem>
              <NFormItem label="窗口置顶">
                <NSwitch v-model:value="options.alwaysOnTop" />
              </NFormItem>
              <NFormItem label="显示触摸点 (-t)">
                <NSwitch v-model:value="options.showTouches" />
              </NFormItem>
              <NFormItem label="关闭音频转发">
                <NSwitch v-model:value="options.noAudio" />
              </NFormItem>
              <NFormItem label="键盘模式">
                <NSelect
                  v-model:value="options.keyboardMode"
                  :options="[
                    { label: 'SDK (默认)', value: 'sdk' },
                    { label: 'UHID (支持中文输入法)', value: 'uhid' },
                  ]"
                />
              </NFormItem>
            </NForm>
          </NCard>
        </NGi>

        <NGi :span="2">
          <NCard size="small" title="虚拟屏与独立应用窗口 (scrcpy 3.0+)" embedded>
            <NSpace align="center" :size="12" wrap>
              <NInput
                v-model:value="options.newDisplay"
                placeholder="--new-display 如 1920x1080/320（留空使用主屏）"
                style="width: 300px"
                clearable
              />
              <NInput
                v-model:value="options.startApp"
                placeholder="--start-app 包名，如 com.tencent.mm"
                style="width: 260px"
                clearable
              />
              <NInput
                v-model:value="options.extraArgs"
                placeholder="额外自定义 CLI 参数"
                style="width: 260px"
                clearable
              />
            </NSpace>
          </NCard>
        </NGi>
      </NGrid>
    </NCard>

    <!-- 运行中的实例 -->
    <NCard title="运行中的投屏实例" size="small">
      <NList v-if="runningInstances.length" bordered size="small">
        <NListItem v-for="inst in runningInstances" :key="inst.instanceId">
          <NSpace justify="space-between" align="center" style="width: 100%">
            <NSpace align="center" :size="8">
              <NTag type="success" size="small">运行中</NTag>
              <strong>{{ inst.serial }}</strong>
              <NTag v-if="inst.profileName" size="tiny">{{ inst.profileName }}</NTag>
            </NSpace>
            <NButton size="tiny" type="error" @click="stopInstance(inst.instanceId)">结束投屏</NButton>
          </NSpace>
        </NListItem>
      </NList>
      <div v-else style="opacity: 0.6; font-size: 13px">当前没有运行中的 scrcpy 实例</div>
    </NCard>
  </NSpace>
</template>

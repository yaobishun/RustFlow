<script setup lang="ts">
defineProps<{ loading?: boolean; error?: string; empty?: boolean; emptyText?: string }>()
defineEmits<{ retry: [] }>()
</script>
<template>
  <div v-if="loading" class="state-panel"><el-skeleton :rows="6" animated /></div>
  <el-result v-else-if="error" icon="error" title="数据加载失败" :sub-title="error">
    <template #extra><el-button type="primary" @click="$emit('retry')">重新加载</el-button></template>
  </el-result>
  <el-empty v-else-if="empty" :description="emptyText || '暂无数据'" />
  <slot v-else />
</template>

